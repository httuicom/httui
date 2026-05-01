//! Block run history — Story 24.6.
//!
//! Stores metadata about HTTP block runs (method, URL canonical, status,
//! sizes, elapsed, timestamp) in SQLite. Body of request/response is NEVER
//! persisted here — privacy-by-default. The drawer reads the last 10 entries
//! per (file_path, alias).
//!
//! Trim policy: after each insert we delete rows for the same
//! (file_path, alias) keeping only the most recent N (default 10). Cap is
//! a private constant — a global retention setting can be wired later.

use sqlx::SqlitePool;

pub mod summary;
pub mod types;

pub use summary::{summarize_last_run, LastRunSummary};
pub use types::{HistoryEntry, InsertEntry};

const DEFAULT_HISTORY_CAP: i64 = 10;
const RETENTION_KEY: &str = "history_retention";

/// Read the user-configured history retention from `app_config`, falling
/// back to the default. Values <= 0 are treated as the default — to fully
/// disable history the per-block `history_disabled` flag is the right
/// switch (Onda 1).
async fn get_retention(pool: &SqlitePool) -> i64 {
    let row: Option<String> = sqlx::query_scalar("SELECT value FROM app_config WHERE key = ?")
        .bind(RETENTION_KEY)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
    match row.and_then(|s| s.parse::<i64>().ok()) {
        Some(n) if n > 0 => n,
        _ => DEFAULT_HISTORY_CAP,
    }
}

/// Insert a new history entry and trim the oldest rows for the same
/// (file_path, alias) so only the most recent `HISTORY_CAP` remain.
pub async fn insert_history_entry(
    pool: &SqlitePool,
    entry: InsertEntry,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO block_run_history (
            file_path, block_alias, method, url_canonical, status,
            request_size, response_size, elapsed_ms, outcome, ran_at, plan
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&entry.file_path)
    .bind(&entry.block_alias)
    .bind(&entry.method)
    .bind(&entry.url_canonical)
    .bind(entry.status)
    .bind(entry.request_size)
    .bind(entry.response_size)
    .bind(entry.elapsed_ms)
    .bind(&entry.outcome)
    .bind(&now)
    .bind(&entry.plan)
    .execute(pool)
    .await?;

    // Trim to the retention cap (user-configurable via app_config; default 10).
    let cap = get_retention(pool).await;
    sqlx::query(
        "DELETE FROM block_run_history
         WHERE file_path = ? AND block_alias = ?
           AND id NOT IN (
             SELECT id FROM block_run_history
             WHERE file_path = ? AND block_alias = ?
             ORDER BY ran_at DESC
             LIMIT ?
           )",
    )
    .bind(&entry.file_path)
    .bind(&entry.block_alias)
    .bind(&entry.file_path)
    .bind(&entry.block_alias)
    .bind(cap)
    .execute(pool)
    .await?;

    Ok(())
}

/// Return the most recent N entries for a (file, alias), most recent first.
pub async fn list_history(
    pool: &SqlitePool,
    file_path: &str,
    block_alias: &str,
) -> Result<Vec<HistoryEntry>, sqlx::Error> {
    let rows = sqlx::query_as::<
        _,
        (
            i64,
            String,
            String,
            String,
            String,
            Option<i64>,
            Option<i64>,
            Option<i64>,
            Option<i64>,
            String,
            String,
            Option<String>,
        ),
    >(
        "SELECT id, file_path, block_alias, method, url_canonical, status,
                request_size, response_size, elapsed_ms, outcome, ran_at, plan
         FROM block_run_history
         WHERE file_path = ? AND block_alias = ?
         ORDER BY ran_at DESC
         LIMIT ?",
    )
    .bind(file_path)
    .bind(block_alias)
    .bind(get_retention(pool).await)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| HistoryEntry {
            id: r.0,
            file_path: r.1,
            block_alias: r.2,
            method: r.3,
            url_canonical: r.4,
            status: r.5,
            request_size: r.6,
            response_size: r.7,
            elapsed_ms: r.8,
            outcome: r.9,
            ran_at: r.10,
            plan: r.11,
        })
        .collect())
}

/// Return the most recent N entries for a single file across all
/// aliases. Used by Epic 29 sidebar History tab to show every run
/// inside the active runbook without an N+1 IPC fan-out from the
/// frontend. Output is most-recent-first; cap is the per-(file,
/// alias) retention multiplied by a small constant so a busy doc
/// with many blocks still surfaces a sensible window.
pub async fn list_history_for_file(
    pool: &SqlitePool,
    file_path: &str,
    limit: i64,
) -> Result<Vec<HistoryEntry>, sqlx::Error> {
    let effective_limit = if limit > 0 { limit } else { 50 };
    let rows = sqlx::query_as::<
        _,
        (
            i64,
            String,
            String,
            String,
            String,
            Option<i64>,
            Option<i64>,
            Option<i64>,
            Option<i64>,
            String,
            String,
            Option<String>,
        ),
    >(
        "SELECT id, file_path, block_alias, method, url_canonical, status,
                request_size, response_size, elapsed_ms, outcome, ran_at, plan
         FROM block_run_history
         WHERE file_path = ?
         ORDER BY ran_at DESC
         LIMIT ?",
    )
    .bind(file_path)
    .bind(effective_limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| HistoryEntry {
            id: r.0,
            file_path: r.1,
            block_alias: r.2,
            method: r.3,
            url_canonical: r.4,
            status: r.5,
            request_size: r.6,
            response_size: r.7,
            elapsed_ms: r.8,
            outcome: r.9,
            ran_at: r.10,
            plan: r.11,
        })
        .collect())
}

/// Delete all history rows for a (file, alias). Called when a block is
/// deleted from the document or a note is removed.
pub async fn purge_history(
    pool: &SqlitePool,
    file_path: &str,
    block_alias: &str,
) -> Result<u64, sqlx::Error> {
    let result =
        sqlx::query("DELETE FROM block_run_history WHERE file_path = ? AND block_alias = ?")
            .bind(file_path)
            .bind(block_alias)
            .execute(pool)
            .await?;
    Ok(result.rows_affected())
}

/// Cascade-delete every history row for a file. Called from the
/// `delete_note` Tauri command when the host note is removed.
pub async fn purge_history_for_file(
    pool: &SqlitePool,
    file_path: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM block_run_history WHERE file_path = ?")
        .bind(file_path)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        // Apply the migration manually for tests.
        sqlx::query(
            "CREATE TABLE block_run_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL,
                block_alias TEXT NOT NULL,
                method TEXT NOT NULL,
                url_canonical TEXT NOT NULL,
                status INTEGER,
                request_size INTEGER,
                response_size INTEGER,
                elapsed_ms INTEGER,
                outcome TEXT NOT NULL,
                ran_at TEXT NOT NULL,
                plan TEXT
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    fn entry(method: &str, status: i64) -> InsertEntry {
        InsertEntry {
            file_path: "/notes/test.md".to_string(),
            block_alias: "req1".to_string(),
            method: method.to_string(),
            url_canonical: "https://api.example.com/users".to_string(),
            status: Some(status),
            request_size: Some(0),
            response_size: Some(42),
            elapsed_ms: Some(100),
            outcome: "success".to_string(),
            plan: None,
        }
    }

    #[tokio::test]
    async fn inserts_and_lists() {
        let pool = setup().await;
        insert_history_entry(&pool, entry("GET", 200))
            .await
            .unwrap();
        insert_history_entry(&pool, entry("POST", 201))
            .await
            .unwrap();
        let rows = list_history(&pool, "/notes/test.md", "req1").await.unwrap();
        assert_eq!(rows.len(), 2);
        // Most recent first → POST was inserted last.
        assert_eq!(rows[0].method, "POST");
        assert_eq!(rows[1].method, "GET");
    }

    #[tokio::test]
    async fn trims_to_history_cap() {
        let pool = setup().await;
        for i in 0..15 {
            // Status doubles as an ordinal so we can identify which rows survived.
            insert_history_entry(&pool, entry("GET", 200 + i))
                .await
                .unwrap();
        }
        let rows = list_history(&pool, "/notes/test.md", "req1").await.unwrap();
        assert_eq!(rows.len(), DEFAULT_HISTORY_CAP as usize);
        // Newest 10 should be statuses 205..=214.
        let statuses: Vec<i64> = rows.iter().map(|r| r.status.unwrap()).collect();
        assert_eq!(statuses[0], 214);
        assert_eq!(statuses[9], 205);
    }

    #[tokio::test]
    async fn isolates_by_file_and_alias() {
        let pool = setup().await;
        let mut e = entry("GET", 200);
        e.file_path = "/a.md".to_string();
        insert_history_entry(&pool, e).await.unwrap();
        let mut e = entry("GET", 200);
        e.file_path = "/b.md".to_string();
        insert_history_entry(&pool, e).await.unwrap();
        let mut e = entry("GET", 200);
        e.block_alias = "other".to_string();
        insert_history_entry(&pool, e).await.unwrap();

        assert_eq!(list_history(&pool, "/a.md", "req1").await.unwrap().len(), 1);
        assert_eq!(list_history(&pool, "/b.md", "req1").await.unwrap().len(), 1);
        assert_eq!(
            list_history(&pool, "/notes/test.md", "other")
                .await
                .unwrap()
                .len(),
            1,
        );
    }

    #[tokio::test]
    async fn plan_blob_round_trips_when_present() {
        let pool = setup().await;
        let mut e = entry("POST", 200);
        e.plan = Some(r#"[{"Plan":{"Node Type":"Seq Scan"}}]"#.into());
        insert_history_entry(&pool, e).await.unwrap();
        let rows = list_history(&pool, "/notes/test.md", "req1").await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].plan.as_deref(),
            Some(r#"[{"Plan":{"Node Type":"Seq Scan"}}]"#),
        );
    }

    #[tokio::test]
    async fn plan_defaults_to_none_for_regular_runs() {
        let pool = setup().await;
        insert_history_entry(&pool, entry("GET", 200)).await.unwrap();
        let rows = list_history(&pool, "/notes/test.md", "req1").await.unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].plan.is_none());
    }

    #[tokio::test]
    async fn plan_serializes_with_skip_none() {
        let entry = HistoryEntry {
            id: 1,
            file_path: "x.md".into(),
            block_alias: "a".into(),
            method: "GET".into(),
            url_canonical: "/".into(),
            status: Some(200),
            request_size: None,
            response_size: None,
            elapsed_ms: None,
            outcome: "success".into(),
            ran_at: "2026-04-30T16:00:00Z".into(),
            plan: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        // None fields are skipped so the existing TS interface
        // (which doesn't yet declare `plan?: string`) keeps
        // accepting the same shape.
        assert!(!json.contains("\"plan\""));

        let with_plan = HistoryEntry {
            plan: Some("{}".into()),
            ..entry
        };
        let json2 = serde_json::to_string(&with_plan).unwrap();
        assert!(json2.contains("\"plan\":\"{}\""));
    }

    #[tokio::test]
    async fn purge_removes_block_history() {
        let pool = setup().await;
        for _ in 0..3 {
            insert_history_entry(&pool, entry("GET", 200))
                .await
                .unwrap();
        }
        let removed = purge_history(&pool, "/notes/test.md", "req1")
            .await
            .unwrap();
        assert_eq!(removed, 3);
        let rows = list_history(&pool, "/notes/test.md", "req1").await.unwrap();
        assert!(rows.is_empty());
    }

    #[tokio::test]
    async fn purge_history_for_file_removes_all_aliases_in_file() {
        let pool = setup().await;
        let mut a = entry("GET", 200);
        a.block_alias = "alpha".into();
        insert_history_entry(&pool, a).await.unwrap();
        let mut b = entry("GET", 200);
        b.block_alias = "beta".into();
        insert_history_entry(&pool, b).await.unwrap();
        let mut other = entry("GET", 200);
        other.file_path = "/other.md".into();
        insert_history_entry(&pool, other).await.unwrap();

        let removed = purge_history_for_file(&pool, "/notes/test.md")
            .await
            .unwrap();
        assert_eq!(removed, 2);
        // Other file's row survives.
        assert_eq!(
            list_history(&pool, "/other.md", "req1").await.unwrap().len(),
            1,
        );
    }

    #[tokio::test]
    async fn get_retention_uses_app_config_when_set() {
        let pool = setup().await;
        // Create the app_config table that `get_retention` reads.
        sqlx::query("CREATE TABLE app_config (key TEXT PRIMARY KEY, value TEXT)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO app_config (key, value) VALUES (?, ?)")
            .bind(RETENTION_KEY)
            .bind("3")
            .execute(&pool)
            .await
            .unwrap();
        // Insert 5 rows; trim should keep only the configured 3.
        for i in 0..5 {
            insert_history_entry(&pool, entry("GET", 200 + i))
                .await
                .unwrap();
        }
        let rows = list_history(&pool, "/notes/test.md", "req1").await.unwrap();
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn insert_entry_round_trips_through_serde_with_plan_default() {
        let json = r#"{
            "file_path": "x.md",
            "block_alias": "a",
            "method": "GET",
            "url_canonical": "/",
            "status": 200,
            "request_size": null,
            "response_size": 12,
            "elapsed_ms": 100,
            "outcome": "success"
        }"#;
        let parsed: InsertEntry = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.method, "GET");
        assert!(parsed.plan.is_none(),
                "plan should default to None when omitted from wire");
    }

    #[test]
    fn insert_entry_accepts_explicit_plan_field() {
        let json = r#"{
            "file_path": "x.md",
            "block_alias": "a",
            "method": "POST",
            "url_canonical": "/q",
            "status": 200,
            "request_size": null,
            "response_size": null,
            "elapsed_ms": null,
            "outcome": "success",
            "plan": "[{\"Plan\":{}}]"
        }"#;
        let parsed: InsertEntry = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.plan.as_deref(), Some(r#"[{"Plan":{}}]"#));
    }

    #[test]
    fn history_entry_clones_with_all_fields() {
        let original = HistoryEntry {
            id: 1,
            file_path: "x.md".into(),
            block_alias: "a".into(),
            method: "GET".into(),
            url_canonical: "/".into(),
            status: Some(200),
            request_size: Some(0),
            response_size: Some(42),
            elapsed_ms: Some(100),
            outcome: "success".into(),
            ran_at: "2026-04-30T16:00:00Z".into(),
            plan: Some("[]".into()),
        };
        let cloned = original.clone();
        assert_eq!(cloned.id, 1);
        assert_eq!(cloned.plan.as_deref(), Some("[]"));
    }

    #[tokio::test]
    async fn list_history_for_file_aggregates_across_aliases() {
        let pool = setup().await;
        // Insert 2 entries for alpha + 1 for beta in the same file.
        let mut a = entry("GET", 200);
        a.block_alias = "alpha".into();
        insert_history_entry(&pool, a.clone()).await.unwrap();
        insert_history_entry(&pool, a).await.unwrap();
        let mut b = entry("POST", 201);
        b.block_alias = "beta".into();
        insert_history_entry(&pool, b).await.unwrap();
        // A row in a different file shouldn't surface.
        let mut other = entry("GET", 200);
        other.file_path = "/other.md".into();
        other.block_alias = "alpha".into();
        insert_history_entry(&pool, other).await.unwrap();

        let rows = list_history_for_file(&pool, "/notes/test.md", 50)
            .await
            .unwrap();
        assert_eq!(rows.len(), 3);
        // Most-recent-first: the last inserted one (beta POST) is on top.
        assert_eq!(rows[0].block_alias, "beta");
    }

    #[tokio::test]
    async fn list_history_for_file_respects_explicit_limit() {
        let pool = setup().await;
        for i in 0..6 {
            insert_history_entry(&pool, entry("GET", 200 + i)).await.unwrap();
        }
        let rows = list_history_for_file(&pool, "/notes/test.md", 3)
            .await
            .unwrap();
        assert_eq!(rows.len(), 3);
    }

    #[tokio::test]
    async fn list_history_for_file_falls_back_to_50_when_limit_non_positive() {
        let pool = setup().await;
        for i in 0..3 {
            insert_history_entry(&pool, entry("GET", 200 + i)).await.unwrap();
        }
        // limit <= 0 → effective fallback of 50; 3 inserted rows fit easily.
        let rows = list_history_for_file(&pool, "/notes/test.md", 0)
            .await
            .unwrap();
        assert_eq!(rows.len(), 3);
        let rows_neg = list_history_for_file(&pool, "/notes/test.md", -10)
            .await
            .unwrap();
        assert_eq!(rows_neg.len(), 3);
    }

    #[tokio::test]
    async fn list_history_for_file_returns_empty_when_no_rows() {
        let pool = setup().await;
        let rows = list_history_for_file(&pool, "/empty.md", 50)
            .await
            .unwrap();
        assert!(rows.is_empty());
    }

    #[tokio::test]
    async fn get_retention_falls_back_to_default_for_invalid_value() {
        let pool = setup().await;
        sqlx::query("CREATE TABLE app_config (key TEXT PRIMARY KEY, value TEXT)")
            .execute(&pool)
            .await
            .unwrap();
        // Negative value — treat as default (10).
        sqlx::query("INSERT INTO app_config (key, value) VALUES (?, ?)")
            .bind(RETENTION_KEY)
            .bind("-5")
            .execute(&pool)
            .await
            .unwrap();
        for _ in 0..2 {
            insert_history_entry(&pool, entry("GET", 200)).await.unwrap();
        }
        // 2 rows fits the default cap (10) with room to spare.
        let rows = list_history(&pool, "/notes/test.md", "req1").await.unwrap();
        assert_eq!(rows.len(), 2);

        // Replace with a non-numeric value — same fallback.
        sqlx::query("UPDATE app_config SET value = ? WHERE key = ?")
            .bind("not a number")
            .bind(RETENTION_KEY)
            .execute(&pool)
            .await
            .unwrap();
        for _ in 0..2 {
            insert_history_entry(&pool, entry("GET", 200)).await.unwrap();
        }
        let rows = list_history(&pool, "/notes/test.md", "req1").await.unwrap();
        // 4 inserted total, default cap is 10 — all 4 visible.
        assert_eq!(rows.len(), 4);
    }
}
