pub mod chat;
pub mod connections;
pub mod driver;
pub mod environments;
pub mod keychain;
pub mod lookup;
pub mod pool;
pub mod pool_exec_mysql;
pub mod pool_exec_pg;
pub mod pool_exec_sqlite;
pub mod pool_manager;
pub mod query_error;
pub mod schema_cache;
pub mod schema_cache_remote;
pub mod sql_scanner;

use serde::Serialize;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::{Column, Row, TypeInfo};
use std::path::Path;
use std::str::FromStr;

use connections::{
    contains_multiple_statements, sanitize_query_error, sqlite_row_to_json, ColumnInfo, JsonRow,
};

const MIGRATION_SQL: &str = include_str!("../../migrations/001_initial.sql");
const MIGRATION_002_SQL: &str = include_str!("../../migrations/002_env_is_secret.sql");
const MIGRATION_003_SQL: &str = include_str!("../../migrations/003_chat.sql");
const MIGRATION_004_SQL: &str = include_str!("../../migrations/004_permissions.sql");
const MIGRATION_005_SQL: &str = include_str!("../../migrations/005_audit_log.sql");
const MIGRATION_006_SQL: &str = include_str!("../../migrations/006_schema_cache_schema_name.sql");
const MIGRATION_007_SQL: &str = include_str!("../../migrations/007_connection_readonly.sql");
const MIGRATION_008_SQL: &str = include_str!("../../migrations/008_sqlite_port_null.sql");
const MIGRATION_009_SQL: &str = include_str!("../../migrations/009_block_run_history.sql");
const MIGRATION_010_SQL: &str = include_str!("../../migrations/010_block_settings.sql");
const MIGRATION_011_SQL: &str = include_str!("../../migrations/011_block_examples.sql");
const MIGRATION_012_SQL: &str = include_str!("../../migrations/012_block_run_history_plan.sql");
const MIGRATION_013_SQL: &str = include_str!("../../migrations/013_schema_cache_drop_fk.sql");
const MIGRATION_014_SQL: &str = include_str!("../../migrations/014_block_results_alias.sql");

pub async fn init_db(app_data_dir: &Path) -> Result<SqlitePool, sqlx::Error> {
    std::fs::create_dir_all(app_data_dir).ok();

    let db_path = app_data_dir.join("notes.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let options = SqliteConnectOptions::from_str(&db_url)?
        .create_if_missing(true)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    run_migrations(&pool).await?;

    // Restrict file permissions on Unix (owner-only read/write).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if db_path.exists() {
            let _ = std::fs::set_permissions(&db_path, std::fs::Permissions::from_mode(0o600));
        }
    }

    Ok(pool)
}

// Internal DB query — read-only, for audit/settings UI.
const MAX_INTERNAL_FETCH_SIZE: u32 = 500;
const MAX_INTERNAL_OFFSET: u32 = 100_000;

#[derive(Debug, Serialize)]
pub struct InternalQueryResult {
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub has_more: bool,
}

/// Execute a read-only query against the app's internal SQLite database.
/// Only SELECT, WITH, read-only PRAGMA, and EXPLAIN are allowed.
pub async fn query_internal_db(
    pool: &SqlitePool,
    sql: &str,
    offset: u32,
    fetch_size: u32,
) -> Result<InternalQueryResult, String> {
    let trimmed = sql.trim_start();
    let upper = trimmed.to_uppercase();

    let allowed = upper.starts_with("SELECT")
        || upper.starts_with("WITH")
        || upper.starts_with("EXPLAIN")
        || (upper.starts_with("PRAGMA") && !upper.contains('='));

    if !allowed {
        return Err("Only SELECT queries are allowed on the internal database".to_string());
    }

    if contains_multiple_statements(trimmed) {
        return Err("Multi-statement queries are not allowed".to_string());
    }

    let fetch_size = fetch_size.clamp(1, MAX_INTERNAL_FETCH_SIZE);
    let offset = offset.min(MAX_INTERNAL_OFFSET);

    let limit = (fetch_size + 1) as i64;
    let off = offset as i64;
    let paginated_sql = format!("SELECT * FROM ({trimmed}) LIMIT {limit} OFFSET {off}");

    let mut rows = sqlx::query(&paginated_sql)
        .fetch_all(pool)
        .await
        .map_err(sanitize_query_error)?;

    let has_more = rows.len() > fetch_size as usize;
    if has_more {
        rows.pop();
    }

    let columns: Vec<ColumnInfo> = if let Some(first) = rows.first() {
        first
            .columns()
            .iter()
            .map(|c| ColumnInfo {
                name: c.name().to_string(),
                type_name: c.type_info().name().to_string(),
            })
            .collect()
    } else {
        Vec::new()
    };

    let json_rows: Vec<JsonRow> = rows.iter().map(sqlite_row_to_json).collect();

    Ok(InternalQueryResult {
        columns,
        rows: json_rows,
        has_more,
    })
}

async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    for statement in MIGRATION_SQL.split(';') {
        let trimmed = statement.trim();
        if !trimmed.is_empty() {
            sqlx::query(trimmed).execute(pool).await?;
        }
    }

    // Run incremental migrations (idempotent ALTER TABLE)
    for statement in MIGRATION_002_SQL.split(';') {
        let trimmed = statement.trim();
        if !trimmed.is_empty() {
            // ALTER TABLE may fail if column already exists — that's ok
            let _ = sqlx::query(trimmed).execute(pool).await;
        }
    }

    for statement in MIGRATION_003_SQL.split(';') {
        let trimmed = statement.trim();
        if !trimmed.is_empty() {
            sqlx::query(trimmed).execute(pool).await?;
        }
    }

    for statement in MIGRATION_004_SQL.split(';') {
        let trimmed = statement.trim();
        if !trimmed.is_empty() {
            let _ = sqlx::query(trimmed).execute(pool).await;
        }
    }

    for statement in MIGRATION_005_SQL.split(';') {
        let trimmed = statement.trim();
        if !trimmed.is_empty() {
            let _ = sqlx::query(trimmed).execute(pool).await;
        }
    }

    for statement in MIGRATION_006_SQL.split(';') {
        let trimmed = statement.trim();
        if !trimmed.is_empty() {
            let _ = sqlx::query(trimmed).execute(pool).await;
        }
    }

    for statement in MIGRATION_007_SQL.split(';') {
        let trimmed = statement.trim();
        if !trimmed.is_empty() {
            let _ = sqlx::query(trimmed).execute(pool).await;
        }
    }

    for statement in MIGRATION_008_SQL.split(';') {
        let trimmed = statement.trim();
        if !trimmed.is_empty() {
            let _ = sqlx::query(trimmed).execute(pool).await;
        }
    }

    for statement in MIGRATION_009_SQL.split(';') {
        let trimmed = statement.trim();
        if !trimmed.is_empty() {
            sqlx::query(trimmed).execute(pool).await?;
        }
    }

    for statement in MIGRATION_010_SQL.split(';') {
        let trimmed = statement.trim();
        if !trimmed.is_empty() {
            sqlx::query(trimmed).execute(pool).await?;
        }
    }

    for statement in MIGRATION_011_SQL.split(';') {
        let trimmed = statement.trim();
        if !trimmed.is_empty() {
            sqlx::query(trimmed).execute(pool).await?;
        }
    }

    // ALTER may fail when column already exists — same idempotent pattern as other ALTER migrations.
    for statement in MIGRATION_012_SQL.split(';') {
        let trimmed = statement.trim();
        if !trimmed.is_empty() {
            let _ = sqlx::query(trimmed).execute(pool).await;
        }
    }

    // Migration 013 recreates schema_cache without the connection_id FK.
    // Not idempotent at the SQL layer (DROP + RENAME), so guard at the Rust
    // layer by inspecting pragma_foreign_key_list — only re-run while the FK
    // is still present.
    let fk_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM pragma_foreign_key_list('schema_cache')")
            .fetch_one(pool)
            .await
            .unwrap_or(0);
    if fk_count > 0 {
        for statement in MIGRATION_013_SQL.split(';') {
            let trimmed = statement.trim();
            if !trimmed.is_empty() {
                sqlx::query(trimmed).execute(pool).await?;
            }
        }
    }

    // Migration 014 adds `alias` to block_results so the TUI ref
    // popup can resolve `{{alias.…}}` by latest-by-alias instead of
    // strict hash match. ALTER may fail when column already exists —
    // idempotent.
    for statement in MIGRATION_014_SQL.split(';') {
        let trimmed = statement.trim();
        if !trimmed.is_empty() {
            let _ = sqlx::query(trimmed).execute(pool).await;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_init_db_creates_file_and_runs_migrations() {
        let tmp = TempDir::new().unwrap();
        let pool = init_db(tmp.path()).await.unwrap();

        let result = sqlx::query("SELECT COUNT(*) as count FROM app_config")
            .fetch_one(&pool)
            .await;
        assert!(result.is_ok());

        let result = sqlx::query("SELECT COUNT(*) as count FROM connections")
            .fetch_one(&pool)
            .await;
        assert!(result.is_ok());

        let result = sqlx::query("SELECT COUNT(*) as count FROM environments")
            .fetch_one(&pool)
            .await;
        assert!(result.is_ok());

        let result = sqlx::query("SELECT COUNT(*) as count FROM block_results")
            .fetch_one(&pool)
            .await;
        assert!(result.is_ok());

        let result = sqlx::query("SELECT COUNT(*) as count FROM schema_cache")
            .fetch_one(&pool)
            .await;
        assert!(result.is_ok());

        pool.close().await;
    }

    #[tokio::test]
    async fn test_init_db_is_idempotent() {
        let tmp = TempDir::new().unwrap();

        let pool1 = init_db(tmp.path()).await.unwrap();
        pool1.close().await;

        let pool2 = init_db(tmp.path()).await.unwrap();
        pool2.close().await;
    }

    async fn memory_pool() -> SqlitePool {
        SqlitePool::connect("sqlite::memory:").await.unwrap()
    }

    #[tokio::test]
    async fn query_internal_db_rejects_non_select() {
        let pool = memory_pool().await;
        for sql in [
            "DELETE FROM x",
            "UPDATE x SET y = 1",
            "INSERT INTO x VALUES (1)",
            "DROP TABLE x",
            "PRAGMA journal_mode = WAL", // PRAGMA with `=` writes
        ] {
            let err = query_internal_db(&pool, sql, 0, 10).await.unwrap_err();
            assert!(err.contains("Only SELECT"), "{sql}");
        }
    }

    #[tokio::test]
    async fn query_internal_db_rejects_multi_statement() {
        let pool = memory_pool().await;
        let err = query_internal_db(&pool, "SELECT 1; SELECT 2", 0, 10)
            .await
            .unwrap_err();
        assert!(err.contains("Multi-statement"));
    }

    #[tokio::test]
    async fn query_internal_db_accepts_select_with_in_memory_pool() {
        let pool = memory_pool().await;
        sqlx::query("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO t (name) VALUES ('a'), ('b'), ('c')")
            .execute(&pool)
            .await
            .unwrap();

        let result = query_internal_db(&pool, "SELECT * FROM t", 0, 2)
            .await
            .unwrap();
        assert_eq!(result.rows.len(), 2);
        assert!(result.has_more, "third row should be flagged via has_more");
        assert_eq!(result.columns.len(), 2);
    }

    #[tokio::test]
    async fn query_internal_db_clamps_fetch_size_and_offset() {
        let pool = memory_pool().await;
        sqlx::query("CREATE TABLE t (id INTEGER PRIMARY KEY)")
            .execute(&pool)
            .await
            .unwrap();
        // fetch_size = 0 should clamp to 1, offset = u32::MAX clamps too.
        let result = query_internal_db(&pool, "SELECT * FROM t", u32::MAX, 0)
            .await
            .unwrap();
        // Empty table — just verify no panic on clamp.
        assert!(result.rows.is_empty());
    }

    #[tokio::test]
    async fn query_internal_db_accepts_with_query() {
        // `WITH` (CTE) is in the read-allowlist alongside SELECT.
        let pool = memory_pool().await;
        sqlx::query("CREATE TABLE t (id INTEGER PRIMARY KEY)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO t (id) VALUES (1)")
            .execute(&pool)
            .await
            .unwrap();
        let result = query_internal_db(
            &pool,
            "WITH cte AS (SELECT * FROM t) SELECT * FROM cte",
            0,
            10,
        )
        .await
        .unwrap();
        assert_eq!(result.rows.len(), 1);
    }

    #[tokio::test]
    async fn query_internal_db_surfaces_sql_errors() {
        let pool = memory_pool().await;
        // Querying a table that doesn't exist — sanitize_query_error
        // converts the sqlx error to a user-facing string.
        let err = query_internal_db(&pool, "SELECT * FROM nonexistent", 0, 10)
            .await
            .unwrap_err();
        assert!(err.to_lowercase().contains("query") || err.to_lowercase().contains("table"));
    }
}
