//! Per-block settings for HTTP blocks.
//!
//! Stores transport/UX flags that the user toggles in the block drawer,
//! keyed by `(file_path, block_alias)`. Stored separately from the fence
//! info string so the .md file stays clean — at the cost of copy-paste of
//! a fence between vaults losing the overrides.
//!
//! All flag columns are nullable: `NULL` means "use the default". This
//! lets the table grow with new flags without burning rows for blocks
//! that were never customised.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

/// Settings shape exchanged with the frontend. All fields are optional so a
/// row with only `history_disabled` set doesn't have to spell out four
/// other booleans.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockSettings {
    pub follow_redirects: Option<bool>,
    pub verify_ssl: Option<bool>,
    pub encode_url: Option<bool>,
    pub trim_whitespace: Option<bool>,
    pub history_disabled: Option<bool>,
}

fn bool_to_int(b: Option<bool>) -> Option<i64> {
    b.map(|v| if v { 1 } else { 0 })
}

fn int_to_bool(i: Option<i64>) -> Option<bool> {
    i.map(|v| v != 0)
}

/// Tuple shape mirroring the columns selected by `get_settings`. Each entry
/// is the raw int-or-NULL bit for a single flag, in the order they appear
/// in the SELECT.
type SettingsRow = (
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
);

/// Load settings for a given (file, alias). Missing row → all-`None` shape
/// (frontend treats that as "all defaults").
pub async fn get_settings(
    pool: &SqlitePool,
    file_path: &str,
    block_alias: &str,
) -> Result<BlockSettings, sqlx::Error> {
    let row: Option<SettingsRow> = sqlx::query_as(
        "SELECT follow_redirects, verify_ssl, encode_url, trim_whitespace, history_disabled
         FROM block_settings
         WHERE file_path = ? AND block_alias = ?",
    )
    .bind(file_path)
    .bind(block_alias)
    .fetch_optional(pool)
    .await?;

    Ok(match row {
        None => BlockSettings::default(),
        Some((fr, vs, eu, tw, hd)) => BlockSettings {
            follow_redirects: int_to_bool(fr),
            verify_ssl: int_to_bool(vs),
            encode_url: int_to_bool(eu),
            trim_whitespace: int_to_bool(tw),
            history_disabled: int_to_bool(hd),
        },
    })
}

/// Upsert settings for a (file, alias). Pass `None` for any flag to clear
/// it (i.e. revert to default).
pub async fn upsert_settings(
    pool: &SqlitePool,
    file_path: &str,
    block_alias: &str,
    settings: BlockSettings,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO block_settings (
            file_path, block_alias, follow_redirects, verify_ssl, encode_url,
            trim_whitespace, history_disabled, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(file_path, block_alias) DO UPDATE SET
            follow_redirects = excluded.follow_redirects,
            verify_ssl       = excluded.verify_ssl,
            encode_url       = excluded.encode_url,
            trim_whitespace  = excluded.trim_whitespace,
            history_disabled = excluded.history_disabled,
            updated_at       = excluded.updated_at",
    )
    .bind(file_path)
    .bind(block_alias)
    .bind(bool_to_int(settings.follow_redirects))
    .bind(bool_to_int(settings.verify_ssl))
    .bind(bool_to_int(settings.encode_url))
    .bind(bool_to_int(settings.trim_whitespace))
    .bind(bool_to_int(settings.history_disabled))
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

/// Delete settings for a (file, alias). Called when a block is removed
/// from the document.
pub async fn purge_settings(
    pool: &SqlitePool,
    file_path: &str,
    block_alias: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM block_settings WHERE file_path = ? AND block_alias = ?")
        .bind(file_path)
        .bind(block_alias)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

/// Cascade-delete every settings row for a file. Called from the
/// `delete_note` Tauri command when the host note is removed.
pub async fn purge_settings_for_file(
    pool: &SqlitePool,
    file_path: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM block_settings WHERE file_path = ?")
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
        sqlx::query(
            "CREATE TABLE block_settings (
                file_path        TEXT NOT NULL,
                block_alias      TEXT NOT NULL,
                follow_redirects INTEGER,
                verify_ssl       INTEGER,
                encode_url       INTEGER,
                trim_whitespace  INTEGER,
                history_disabled INTEGER,
                updated_at       TEXT NOT NULL,
                PRIMARY KEY (file_path, block_alias)
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn missing_row_returns_all_none() {
        let pool = setup().await;
        let s = get_settings(&pool, "/a.md", "req1").await.unwrap();
        assert_eq!(s.follow_redirects, None);
        assert_eq!(s.verify_ssl, None);
        assert_eq!(s.encode_url, None);
        assert_eq!(s.trim_whitespace, None);
        assert_eq!(s.history_disabled, None);
    }

    #[tokio::test]
    async fn upsert_inserts_then_updates() {
        let pool = setup().await;

        upsert_settings(
            &pool,
            "/a.md",
            "req1",
            BlockSettings {
                follow_redirects: Some(false),
                verify_ssl: Some(false),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let s = get_settings(&pool, "/a.md", "req1").await.unwrap();
        assert_eq!(s.follow_redirects, Some(false));
        assert_eq!(s.verify_ssl, Some(false));
        assert_eq!(s.history_disabled, None);

        upsert_settings(
            &pool,
            "/a.md",
            "req1",
            BlockSettings {
                history_disabled: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        // Upsert overwrites — previously-set fields revert to None when the
        // new payload omits them.
        let s = get_settings(&pool, "/a.md", "req1").await.unwrap();
        assert_eq!(s.history_disabled, Some(true));
        assert_eq!(s.follow_redirects, None);
    }

    #[tokio::test]
    async fn isolates_by_file_and_alias() {
        let pool = setup().await;
        upsert_settings(
            &pool,
            "/a.md",
            "req1",
            BlockSettings {
                history_disabled: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        upsert_settings(&pool, "/b.md", "req1", BlockSettings::default())
            .await
            .unwrap();

        assert_eq!(
            get_settings(&pool, "/a.md", "req1")
                .await
                .unwrap()
                .history_disabled,
            Some(true)
        );
        assert_eq!(
            get_settings(&pool, "/b.md", "req1")
                .await
                .unwrap()
                .history_disabled,
            None
        );
        assert_eq!(
            get_settings(&pool, "/a.md", "other")
                .await
                .unwrap()
                .history_disabled,
            None
        );
    }

    #[tokio::test]
    async fn purge_removes_row() {
        let pool = setup().await;
        upsert_settings(
            &pool,
            "/a.md",
            "req1",
            BlockSettings {
                follow_redirects: Some(false),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let removed = purge_settings(&pool, "/a.md", "req1").await.unwrap();
        assert_eq!(removed, 1);
        let s = get_settings(&pool, "/a.md", "req1").await.unwrap();
        assert_eq!(s.follow_redirects, None);
    }
}
