//! DB block run-history snapshot + insert helper.

use crate::app::App;
use crate::buffer::Segment;

/// Snapshot of the bits we need to write a `block_run_history` row
/// for a DB block run. Captured up-front (before the result handler
/// mutates state) so the Ok and Err arms can share a single insert
/// path without re-walking the segment list.
#[derive(Clone)]
pub struct DbHistoryMeta {
    pub file_path: String,
    pub block_alias: String,
    /// Stored in the `method` column as `db:<driver>` (e.g.
    /// `db:postgres`). Mirrors how the HTTP path uses `method` as
    /// the request "kind" — same column, different namespace.
    pub method: String,
    /// SQL preview (~200 chars, single-line). Goes in the
    /// `url_canonical` column whose semantic role is "what was
    /// run" — for HTTP that's URL+query; for DB it's the SQL.
    pub url_canonical: String,
    pub request_size: i64,
}

pub fn snapshot_db_history_meta(app: &App, segment_idx: usize) -> Option<DbHistoryMeta> {
    let tab = app.tabs.tabs.get(app.tabs.active())?;
    let file_path = tab.active_leaf().document_path.as_ref()?;
    let file_path = file_path.to_string_lossy().to_string();
    let doc = app.document()?;
    let block = match doc.segments().get(segment_idx)? {
        Segment::Block(b) => b,
        _ => return None,
    };
    if !block.is_db() {
        return None;
    }
    let alias = block
        .alias
        .as_deref()
        .filter(|s| !s.is_empty())?
        .to_string();
    // `block_type` is `db-postgres`, `db-mysql`, `db-sqlite`. Strip
    // the `db-` prefix for the namespaced method label so the
    // history modal shows a clean `postgres` / `mysql` / `sqlite`
    // chip instead of repeating `db-` everywhere.
    let driver = block
        .block_type
        .strip_prefix("db-")
        .unwrap_or(&block.block_type);
    let method = format!("db:{driver}");

    let query = block
        .params
        .get("query")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let url_canonical = preview_sql(&query);
    let request_size = query.len() as i64;
    Some(DbHistoryMeta {
        file_path,
        block_alias: alias,
        method,
        url_canonical,
        request_size,
    })
}

/// Collapse newlines to spaces and trim to ~200 chars so the
/// history modal's row stays readable. Suffix `…` when truncated.
pub(crate) fn preview_sql(sql: &str) -> String {
    const MAX: usize = 200;
    let collapsed = sql
        .replace(['\r', '\n', '\t'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if collapsed.chars().count() > MAX {
        let truncated: String = collapsed.chars().take(MAX).collect();
        format!("{truncated}…")
    } else {
        collapsed
    }
}

/// Derive the `(status, response_size)` columns from a successful
/// DB response. Status borrows the column for "result kind size":
/// `rows.len()` for a SELECT, `rows_affected` for a mutation,
/// `None` for an error result. Response size is the serialized
/// JSON length — coarse but useful for spotting regressions.
pub fn derive_db_history_stats(
    response: &httui_core::executor::db::types::DbResponse,
) -> (Option<i64>, Option<i64>) {
    use httui_core::executor::db::types::DbResult;
    let status = match response.results.first() {
        Some(DbResult::Select { rows, .. }) => Some(rows.len() as i64),
        Some(DbResult::Mutation { rows_affected }) => Some(*rows_affected as i64),
        _ => None,
    };
    let response_size = serde_json::to_string(response).ok().map(|s| s.len() as i64);
    (status, response_size)
}

/// Spawn the SQLite insert in the background (mirrors the HTTP
/// path). Failures land in the tracing log — history is best-
/// effort and never blocks the user.
pub fn record_db_history_async(
    pool: sqlx::SqlitePool,
    meta: DbHistoryMeta,
    elapsed_ms: Option<i64>,
    status: Option<i64>,
    response_size: Option<i64>,
    outcome: &'static str,
) {
    let entry = httui_core::block_history::InsertEntry {
        file_path: meta.file_path,
        block_alias: meta.block_alias,
        method: meta.method,
        url_canonical: meta.url_canonical,
        status,
        request_size: Some(meta.request_size),
        response_size,
        elapsed_ms,
        outcome: outcome.to_string(),
        plan: None,
    };
    tokio::spawn(async move {
        if let Err(e) = httui_core::block_history::insert_history_entry(&pool, entry).await {
            tracing::warn!("db block history insert failed: {e}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::buffer::{Cursor, Document};
    use crate::config::Config;
    use crate::pane::{Pane, TabState};
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use httui_core::executor::db::types::{DbResponse, DbResult};
    use tempfile::TempDir;

    async fn app_with_doc(
        md: &str,
        with_path: bool,
    ) -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault { vault: vault.path().to_path_buf() };
        let mut app = App::new(Config::default(), resolved, pool);
        let doc = Document::from_markdown(md).unwrap();
        let pane = if with_path {
            let p = vault.path().join("note.md");
            std::fs::write(&p, md).unwrap();
            Pane::new(doc, p)
        } else {
            Pane {
                document: Some(doc),
                document_path: None,
                viewport_top: 0,
                viewport_height: 0,
            }
        };
        app.tabs.tabs.clear();
        app.tabs.tabs.push(TabState::new(pane));
        app.tabs.active = 0;
        (app, data, vault)
    }

    fn first_block(app: &App) -> usize {
        app.document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .expect("block")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn snapshot_returns_some_for_db_block_with_alias_and_path() {
        let md = "```db-sqlite alias=q\nSELECT 1;\n```\n";
        let (app, _d, _v) = app_with_doc(md, true).await;
        let idx = first_block(&app);
        let meta = snapshot_db_history_meta(&app, idx).expect("some");
        assert_eq!(meta.block_alias, "q");
        assert_eq!(meta.method, "db:sqlite");
        assert!(meta.url_canonical.contains("SELECT 1"));
        assert!(meta.request_size > 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn snapshot_returns_none_without_file_path() {
        let md = "```db-sqlite alias=q\nSELECT 1;\n```\n";
        let (app, _d, _v) = app_with_doc(md, false).await;
        let idx = first_block(&app);
        assert!(snapshot_db_history_meta(&app, idx).is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn snapshot_returns_none_for_http_block() {
        let md = "```http alias=a\nGET /x\n```\n";
        let (app, _d, _v) = app_with_doc(md, true).await;
        let idx = first_block(&app);
        assert!(snapshot_db_history_meta(&app, idx).is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn snapshot_returns_none_for_anonymous_db_block() {
        let md = "```db-sqlite\nSELECT 1;\n```\n";
        let (app, _d, _v) = app_with_doc(md, true).await;
        let idx = first_block(&app);
        assert!(snapshot_db_history_meta(&app, idx).is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn snapshot_returns_none_for_segment_that_is_not_a_block() {
        let md = "just prose\n";
        let (app, _d, _v) = app_with_doc(md, true).await;
        assert!(snapshot_db_history_meta(&app, 0).is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn snapshot_returns_none_for_out_of_range_idx() {
        let md = "```db-sqlite alias=q\nSELECT 1;\n```\n";
        let (app, _d, _v) = app_with_doc(md, true).await;
        assert!(snapshot_db_history_meta(&app, 999).is_none());
    }

    #[test]
    fn preview_sql_collapses_whitespace_and_truncates() {
        let short = preview_sql("SELECT\n  *\nFROM t");
        assert_eq!(short, "SELECT * FROM t");
        let long: String = "a ".repeat(200);
        let trimmed = preview_sql(&long);
        assert!(trimmed.ends_with('…'));
        assert!(trimmed.chars().count() <= 201);
    }

    fn stats() -> httui_core::executor::db::types::DbStats {
        httui_core::executor::db::types::DbStats { elapsed_ms: 5, rows_streamed: None }
    }

    fn col(name: &str) -> httui_core::db::connections::ColumnInfo {
        httui_core::db::connections::ColumnInfo {
            name: name.into(),
            type_name: "TEXT".into(),
        }
    }

    #[test]
    fn derive_stats_select_returns_row_count() {
        let response = DbResponse {
            results: vec![DbResult::Select {
                columns: vec![col("a")],
                rows: vec![serde_json::json!([1]), serde_json::json!([2])],
                has_more: false,
            }],
            messages: Vec::new(),
            plan: None,
            stats: stats(),
        };
        let (status, size) = derive_db_history_stats(&response);
        assert_eq!(status, Some(2));
        assert!(size.is_some());
    }

    #[test]
    fn derive_stats_mutation_returns_rows_affected() {
        let response = DbResponse {
            results: vec![DbResult::Mutation { rows_affected: 7 }],
            messages: Vec::new(),
            plan: None,
            stats: stats(),
        };
        let (status, _) = derive_db_history_stats(&response);
        assert_eq!(status, Some(7));
    }

    #[test]
    fn derive_stats_empty_returns_none_status() {
        let response = DbResponse {
            results: vec![],
            messages: Vec::new(),
            plan: None,
            stats: stats(),
        };
        let (status, _) = derive_db_history_stats(&response);
        assert!(status.is_none());
    }

    #[test]
    fn derive_stats_error_result_returns_none_status() {
        let response = DbResponse {
            results: vec![DbResult::Error {
                message: "boom".into(),
                line: None,
                column: None,
            }],
            messages: Vec::new(),
            plan: None,
            stats: stats(),
        };
        let (status, _) = derive_db_history_stats(&response);
        assert!(status.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn record_db_history_async_writes_row() {
        let data = TempDir::new().unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let meta = DbHistoryMeta {
            file_path: "/x/note.md".into(),
            block_alias: "q".into(),
            method: "db:sqlite".into(),
            url_canonical: "SELECT 1".into(),
            request_size: 10,
        };
        record_db_history_async(pool.clone(), meta, Some(5), Some(1), Some(40), "ok");
        // Give the spawned task a moment.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let entries = httui_core::block_history::list_history(&pool, "/x/note.md", "q")
            .await
            .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].outcome, "ok");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn snapshot_keeps_query_size_in_bytes() {
        let sql = "SELECT 1, 2, 3";
        let md = format!("```db-sqlite alias=q\n{sql}\n```\n");
        let (app, _d, _v) = app_with_doc(&md, true).await;
        let idx = first_block(&app);
        let meta = snapshot_db_history_meta(&app, idx).expect("some");
        assert_eq!(meta.request_size, sql.len() as i64);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn snapshot_returns_none_when_doc_absent() {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault { vault: vault.path().to_path_buf() };
        let mut app = App::new(Config::default(), resolved, pool);
        // Empty pane (no document).
        app.tabs.tabs.clear();
        app.tabs.tabs.push(TabState::new(Pane::empty()));
        app.tabs.active = 0;
        // active pane has no document_path either, so snapshot returns None early.
        assert!(snapshot_db_history_meta(&app, 0).is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn snapshot_returns_none_when_no_active_tab() {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault { vault: vault.path().to_path_buf() };
        let mut app = App::new(Config::default(), resolved, pool);
        app.tabs.tabs.clear(); // no active tab
        assert!(snapshot_db_history_meta(&app, 0).is_none());
    }

    // Silence unused import in this module-test scope.
    #[allow(dead_code)]
    fn _touch_cursor(_: Cursor) {}
}
