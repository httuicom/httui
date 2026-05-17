use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use uuid::Uuid;

// Pool lifecycle + status emission moved to `db::pool_manager`
// (Epic 20a Story 01 first split). Re-exported here so the existing
// `use httui_core::db::connections::{PoolManager, StatusEmitter}`
// callers keep compiling without a sweeping import rewrite.
pub use super::pool_manager::{HostPortOverride, PoolManager, StatusEmitter};

// `DatabasePool` enum + lifecycle helpers (`create_pool`, builders,
// validators, sanitizer) moved to `db::pool` (Epic 20a Story 01 —
// fourth split). Re-exported here so existing imports compile.
#[cfg(test)]
use super::pool::validate_bind_values;
use super::pool::validate_sqlite_path;
pub use super::pool::DatabasePool;

// --- Connection model ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub host: Option<String>,
    pub port: Option<i64>,
    pub database_name: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub ssl_mode: Option<String>,
    pub timeout_ms: i64,
    pub query_timeout_ms: i64,
    pub ttl_seconds: i64,
    pub max_pool_size: i64,
    pub is_readonly: bool,
    pub last_tested_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Public DTO without password field — safe for Tauri IPC responses.
#[derive(Debug, Clone, Serialize)]
pub struct ConnectionPublic {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub host: Option<String>,
    pub port: Option<i64>,
    pub database_name: Option<String>,
    pub username: Option<String>,
    pub has_password: bool,
    pub ssl_mode: Option<String>,
    pub timeout_ms: i64,
    pub query_timeout_ms: i64,
    pub ttl_seconds: i64,
    pub max_pool_size: i64,
    pub is_readonly: bool,
    pub last_tested_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl Connection {
    pub fn to_public(&self) -> ConnectionPublic {
        ConnectionPublic {
            id: self.id.clone(),
            name: self.name.clone(),
            driver: self.driver.clone(),
            host: self.host.clone(),
            port: self.port,
            database_name: self.database_name.clone(),
            username: self.username.clone(),
            has_password: self.password.as_ref().is_some_and(|p| !p.is_empty()),
            ssl_mode: self.ssl_mode.clone(),
            timeout_ms: self.timeout_ms,
            query_timeout_ms: self.query_timeout_ms,
            ttl_seconds: self.ttl_seconds,
            max_pool_size: self.max_pool_size,
            is_readonly: self.is_readonly,
            last_tested_at: self.last_tested_at.clone(),
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
        }
    }
}

pub async fn list_connections_public(pool: &SqlitePool) -> Result<Vec<ConnectionPublic>, String> {
    let conns = list_connections(pool).await?;
    Ok(conns.into_iter().map(|c| c.to_public()).collect())
}

#[derive(Debug, Deserialize)]
pub struct CreateConnection {
    pub name: String,
    pub driver: String,
    pub host: Option<String>,
    pub port: Option<i64>,
    pub database_name: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub ssl_mode: Option<String>,
    pub timeout_ms: Option<i64>,
    pub query_timeout_ms: Option<i64>,
    pub ttl_seconds: Option<i64>,
    pub max_pool_size: Option<i64>,
    pub is_readonly: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateConnection {
    pub name: Option<String>,
    pub driver: Option<String>,
    pub host: Option<String>,
    pub port: Option<i64>,
    pub database_name: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub ssl_mode: Option<String>,
    pub timeout_ms: Option<i64>,
    pub query_timeout_ms: Option<i64>,
    pub ttl_seconds: Option<i64>,
    pub max_pool_size: Option<i64>,
    pub is_readonly: Option<bool>,
}

// Query error sanitization + location extraction moved to
// `db::query_error` (Epic 20a Story 01 — second split). Re-exports
// keep existing imports compiling.
pub(crate) use super::query_error::sanitize_query_error;
pub use super::query_error::{
    enrich_error_with_query, sanitize_query_error_rich, QueryErrorInfo, QueryErrorLocation,
};

// --- Row mapping ---

fn row_to_connection(row: &sqlx::sqlite::SqliteRow) -> Connection {
    Connection {
        id: row.get("id"),
        name: row.get("name"),
        driver: row.get("driver"),
        host: row.get("host"),
        port: row.get("port"),
        database_name: row.get("database_name"),
        username: row.get("username"),
        password: row.get("password"),
        ssl_mode: row.get("ssl_mode"),
        timeout_ms: row.get("timeout_ms"),
        query_timeout_ms: row.get("query_timeout_ms"),
        ttl_seconds: row.get("ttl_seconds"),
        max_pool_size: row.get("max_pool_size"),
        // is_readonly stored as INTEGER (0/1) in SQLite
        is_readonly: row
            .try_get::<i64, _>("is_readonly")
            .map(|v| v != 0)
            .unwrap_or(false),
        last_tested_at: row.get("last_tested_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

// --- CRUD functions ---

pub async fn list_connections(pool: &SqlitePool) -> Result<Vec<Connection>, String> {
    let rows = sqlx::query(
        r#"SELECT
            id, name, driver, host, port, database_name, username, password,
            ssl_mode, timeout_ms, query_timeout_ms, ttl_seconds, max_pool_size,
            is_readonly,
            last_tested_at, created_at, updated_at
        FROM connections
        ORDER BY name"#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows.iter().map(row_to_connection).collect())
}

pub async fn get_connection(pool: &SqlitePool, id: &str) -> Result<Option<Connection>, String> {
    let row = sqlx::query(
        r#"SELECT
            id, name, driver, host, port, database_name, username, password,
            ssl_mode, timeout_ms, query_timeout_ms, ttl_seconds, max_pool_size,
            is_readonly,
            last_tested_at, created_at, updated_at
        FROM connections WHERE id = ?"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(row.as_ref().map(row_to_connection))
}

pub async fn create_connection(
    pool: &SqlitePool,
    input: CreateConnection,
) -> Result<Connection, String> {
    validate_connection_fields(
        &input.driver,
        &input.host,
        &input.port,
        &input.database_name,
    )?;

    let id = Uuid::new_v4().to_string();
    let ssl_mode = input.ssl_mode.unwrap_or_else(|| "disable".to_string());
    let timeout_ms = input.timeout_ms.unwrap_or(10000);
    let query_timeout_ms = input.query_timeout_ms.unwrap_or(30000);
    let ttl_seconds = input.ttl_seconds.unwrap_or(300);
    let max_pool_size = input.max_pool_size.unwrap_or(5);

    // Store password in keychain — fail on error, no plaintext fallback
    let db_password = if let Some(ref pw) = input.password {
        if !pw.is_empty() {
            use super::keychain::{conn_password_key, store_secret, KEYCHAIN_SENTINEL};
            store_secret(&conn_password_key(&id), pw)
                .map_err(|e| format!("Failed to store password securely: {e}"))?;
            Some(KEYCHAIN_SENTINEL.to_string())
        } else {
            input.password.clone()
        }
    } else {
        None
    };

    let is_readonly = input.is_readonly.unwrap_or(false);

    sqlx::query(
        r#"INSERT INTO connections
            (id, name, driver, host, port, database_name, username, password,
             ssl_mode, timeout_ms, query_timeout_ms, ttl_seconds, max_pool_size,
             is_readonly)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&id)
    .bind(&input.name)
    .bind(&input.driver)
    .bind(&input.host)
    .bind(input.port)
    .bind(&input.database_name)
    .bind(&input.username)
    .bind(&db_password)
    .bind(&ssl_mode)
    .bind(timeout_ms)
    .bind(query_timeout_ms)
    .bind(ttl_seconds)
    .bind(max_pool_size)
    .bind(if is_readonly { 1i64 } else { 0i64 })
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    get_connection(pool, &id)
        .await?
        .ok_or_else(|| "Failed to fetch created connection".to_string())
}

pub async fn update_connection(
    pool: &SqlitePool,
    id: &str,
    input: UpdateConnection,
) -> Result<Connection, String> {
    let existing = get_connection(pool, id)
        .await?
        .ok_or_else(|| format!("Connection '{}' not found", id))?;

    let name = input.name.unwrap_or(existing.name);
    let driver = input.driver.unwrap_or(existing.driver);
    // Preserve NULLs across partial updates: SQLite has no host/port, so
    // forcing `Some(existing.port.unwrap_or(0))` would fail validation on
    // every partial-field update (like the drawer's read-only toggle).
    let host = input.host.or(existing.host);
    let port = input.port.or(existing.port);
    let database_name = input.database_name.or(existing.database_name);
    let username = input.username.or(existing.username);
    // If a new password is provided, store in keychain — fail on error, no plaintext fallback
    let password = if let Some(ref new_pw) = input.password {
        if !new_pw.is_empty() {
            use super::keychain::{conn_password_key, store_secret, KEYCHAIN_SENTINEL};
            store_secret(&conn_password_key(id), new_pw)
                .map_err(|e| format!("Failed to store password securely: {e}"))?;
            Some(KEYCHAIN_SENTINEL.to_string())
        } else {
            Some(String::new())
        }
    } else {
        existing.password // keep existing (may already be sentinel)
    };
    let ssl_mode = Some(
        input
            .ssl_mode
            .unwrap_or_else(|| existing.ssl_mode.unwrap_or_else(|| "disable".to_string())),
    );
    let timeout_ms = input.timeout_ms.unwrap_or(existing.timeout_ms);
    let query_timeout_ms = input.query_timeout_ms.unwrap_or(existing.query_timeout_ms);
    let ttl_seconds = input.ttl_seconds.unwrap_or(existing.ttl_seconds);
    let max_pool_size = input.max_pool_size.unwrap_or(existing.max_pool_size);
    let is_readonly = input.is_readonly.unwrap_or(existing.is_readonly);

    validate_connection_fields(&driver, &host, &port, &database_name)?;

    sqlx::query(
        r#"UPDATE connections SET
            name = ?, driver = ?, host = ?, port = ?, database_name = ?,
            username = ?, password = ?, ssl_mode = ?, timeout_ms = ?,
            query_timeout_ms = ?, ttl_seconds = ?, max_pool_size = ?,
            is_readonly = ?,
            updated_at = datetime('now')
        WHERE id = ?"#,
    )
    .bind(&name)
    .bind(&driver)
    .bind(&host)
    .bind(port)
    .bind(&database_name)
    .bind(&username)
    .bind(&password)
    .bind(&ssl_mode)
    .bind(timeout_ms)
    .bind(query_timeout_ms)
    .bind(ttl_seconds)
    .bind(max_pool_size)
    .bind(if is_readonly { 1i64 } else { 0i64 })
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    get_connection(pool, id)
        .await?
        .ok_or_else(|| "Failed to fetch updated connection".to_string())
}

pub async fn delete_connection(pool: &SqlitePool, id: &str) -> Result<(), String> {
    let result = sqlx::query("DELETE FROM connections WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    if result.rows_affected() == 0 {
        return Err(format!("Connection '{}' not found", id));
    }

    // Clean up keychain entry (ignore errors — may not exist)
    use super::keychain::{conn_password_key, delete_secret};
    let _ = delete_secret(&conn_password_key(id));

    Ok(())
}

// --- Validation ---

fn validate_connection_fields(
    driver: &str,
    host: &Option<String>,
    _port: &Option<i64>,
    database_name: &Option<String>,
) -> Result<(), String> {
    match driver {
        "postgres" | "mysql" => {
            if host.as_ref().map_or(true, |h| h.is_empty()) {
                return Err(format!("host is required for {driver}"));
            }
            if database_name.as_ref().map_or(true, |d| d.is_empty()) {
                return Err(format!("database_name is required for {driver}"));
            }
        }
        "sqlite" => {
            if database_name.as_ref().map_or(true, |d| d.is_empty()) {
                return Err("database_name (file path) is required for sqlite".to_string());
            }
            if let Some(ref path) = database_name {
                validate_sqlite_path(path)?;
            }
        }
        other => return Err(format!("Unsupported driver: {other}")),
    }
    Ok(())
}

// Query execution dispatcher + DTOs + bind validation moved to
// `db::pool` (Epic 20a Story 01 — eighth split). Re-exports keep
// `httui_core::db::connections::{QueryResult, ColumnInfo, JsonRow}`
// working for downstream consumers (executor/db, blocks/db_export).
pub use super::pool::{ColumnInfo, JsonRow, QueryResult};
pub(crate) use super::pool_exec_sqlite::sqlite_row_to_json;

// SQL scanner + statement splitter + placeholder helpers moved to
// `db::sql_scanner` (Epic 20a Story 01 — third split). Re-exports
// keep existing imports compiling.
pub(crate) use super::sql_scanner::contains_multiple_statements;
pub use super::sql_scanner::{count_placeholders, normalize_placeholders_to_pg, split_statements};

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_test_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Failed to create test pool");

        sqlx::query(
            r#"CREATE TABLE connections (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                driver TEXT NOT NULL CHECK (driver IN ('postgres', 'mysql', 'sqlite')),
                host TEXT,
                port INTEGER,
                database_name TEXT,
                username TEXT,
                password TEXT,
                ssl_mode TEXT DEFAULT 'disable',
                timeout_ms INTEGER DEFAULT 10000,
                query_timeout_ms INTEGER DEFAULT 30000,
                ttl_seconds INTEGER DEFAULT 300,
                max_pool_size INTEGER DEFAULT 5,
                is_readonly INTEGER NOT NULL DEFAULT 0,
                last_tested_at TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )"#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create connections table");

        pool
    }

    #[tokio::test]
    async fn test_create_and_list_connections() {
        let pool = setup_test_pool().await;

        let conn = create_connection(
            &pool,
            CreateConnection {
                name: "test-pg".to_string(),
                driver: "postgres".to_string(),
                host: Some("localhost".to_string()),
                port: Some(5432),
                database_name: Some("testdb".to_string()),
                username: Some("user".to_string()),
                password: Some("pass".to_string()),
                ssl_mode: None,
                timeout_ms: None,
                query_timeout_ms: None,
                ttl_seconds: None,
                max_pool_size: None,
                is_readonly: None,
            },
        )
        .await
        .expect("Failed to create connection");

        assert_eq!(conn.name, "test-pg");
        assert_eq!(conn.driver, "postgres");
        assert_eq!(conn.host.as_deref(), Some("localhost"));

        let all = list_connections(&pool).await.expect("Failed to list");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "test-pg");
    }

    #[tokio::test]
    async fn test_update_connection() {
        let pool = setup_test_pool().await;

        let conn = create_connection(
            &pool,
            CreateConnection {
                name: "my-conn".to_string(),
                driver: "postgres".to_string(),
                host: Some("localhost".to_string()),
                port: Some(5432),
                database_name: Some("db1".to_string()),
                username: None,
                password: None,
                ssl_mode: None,
                timeout_ms: None,
                query_timeout_ms: None,
                ttl_seconds: None,
                max_pool_size: None,
                is_readonly: None,
            },
        )
        .await
        .unwrap();

        let updated = update_connection(
            &pool,
            &conn.id,
            UpdateConnection {
                name: Some("renamed".to_string()),
                driver: None,
                host: None,
                port: None,
                database_name: None,
                username: None,
                password: None,
                ssl_mode: None,
                timeout_ms: None,
                query_timeout_ms: None,
                ttl_seconds: None,
                max_pool_size: None,
                is_readonly: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(updated.name, "renamed");
        assert_eq!(updated.driver, "postgres");
    }

    #[tokio::test]
    async fn test_delete_connection() {
        let pool = setup_test_pool().await;

        let conn = create_connection(
            &pool,
            CreateConnection {
                name: "to-delete".to_string(),
                driver: "sqlite".to_string(),
                host: None,
                port: None,
                database_name: Some("/tmp/test.db".to_string()),
                username: None,
                password: None,
                ssl_mode: None,
                timeout_ms: None,
                query_timeout_ms: None,
                ttl_seconds: None,
                max_pool_size: None,
                is_readonly: None,
            },
        )
        .await
        .unwrap();

        delete_connection(&pool, &conn.id).await.unwrap();

        let all = list_connections(&pool).await.unwrap();
        assert!(all.is_empty());
    }

    #[tokio::test]
    async fn test_validate_postgres_requires_host() {
        let result =
            validate_connection_fields("postgres", &None, &Some(5432), &Some("db".to_string()));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("host is required"));
    }

    #[tokio::test]
    async fn test_validate_sqlite_requires_path() {
        let result = validate_connection_fields("sqlite", &None, &None, &None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("database_name"));
    }

    // build_pg_connect_options / validate_sqlite_path /
    // validate_mysql_database_name / normalize_placeholders tests
    // moved alongside the implementations (`db::pool` and
    // `db::sql_scanner`) — Epic 20a Story 01.

    #[tokio::test]
    async fn test_execute_select_sqlite() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

        sqlx::query("CREATE TABLE test_table (id INTEGER PRIMARY KEY, name TEXT, value REAL)")
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO test_table VALUES (1, 'alpha', 1.5), (2, 'beta', 2.5), (3, 'gamma', 3.5)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let db_pool = DatabasePool::Sqlite(pool);
        let result = db_pool
            .execute_query("SELECT * FROM test_table", &[], 0, 100)
            .await
            .unwrap();

        assert!(result.is_select);
        assert!(!result.has_more);
        assert_eq!(result.rows.len(), 3);
        assert_eq!(result.columns.len(), 3);
        assert_eq!(result.columns[0].name, "id");
        assert_eq!(result.columns[1].name, "name");
    }

    #[tokio::test]
    async fn test_execute_select_with_pagination() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

        sqlx::query("CREATE TABLE items (id INTEGER PRIMARY KEY)")
            .execute(&pool)
            .await
            .unwrap();

        for i in 1..=10 {
            sqlx::query("INSERT INTO items VALUES (?)")
                .bind(i)
                .execute(&pool)
                .await
                .unwrap();
        }

        let db_pool = DatabasePool::Sqlite(pool);

        // offset=0, fetch_size=3 → 3 rows, has_more=true
        let result = db_pool
            .execute_query("SELECT * FROM items", &[], 0, 3)
            .await
            .unwrap();
        assert!(result.has_more);
        assert_eq!(result.rows.len(), 3);

        // offset=9, fetch_size=3 → 1 row, has_more=false
        let result = db_pool
            .execute_query("SELECT * FROM items", &[], 9, 3)
            .await
            .unwrap();
        assert!(!result.has_more);
        assert_eq!(result.rows.len(), 1);
    }

    #[tokio::test]
    async fn test_execute_mutation_sqlite() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

        sqlx::query("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT)")
            .execute(&pool)
            .await
            .unwrap();

        let db_pool = DatabasePool::Sqlite(pool);
        let result = db_pool
            .execute_query("INSERT INTO items VALUES (1, 'test')", &[], 1, 100)
            .await
            .unwrap();

        assert!(!result.is_select);
        assert_eq!(result.rows_affected, Some(1));
    }

    #[tokio::test]
    async fn test_execute_with_bind_params() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

        sqlx::query("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, active INTEGER)")
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query("INSERT INTO users VALUES (1, 'alice', 1), (2, 'bob', 0), (3, 'charlie', 1)")
            .execute(&pool)
            .await
            .unwrap();

        let db_pool = DatabasePool::Sqlite(pool);
        let result = db_pool
            .execute_query(
                "SELECT * FROM users WHERE active = ? AND name != ?",
                &[serde_json::json!(1), serde_json::json!("alice")],
                0,
                100,
            )
            .await
            .unwrap();

        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0][1], serde_json::json!("charlie"));
    }

    // Pool config validation tests moved to `db::pool::tests`
    // (Epic 20a Story 01 — fourth extraction).

    // SQL-scanner / split_statements / contains_multiple_statements
    // tests moved to `db::sql_scanner::tests` along with the
    // implementations (Epic 20a Story 01 — third extraction).

    // QueryErrorLocation / position_to_line_col / mysql_line_from_message
    // tests moved to `db::query_error::tests` along with the functions
    // (Epic 20a Story 01 — second extraction).

    #[tokio::test]
    async fn test_explain_analyze_delete_rejected() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("CREATE TABLE t (id INTEGER)")
            .execute(&pool)
            .await
            .unwrap();
        let db_pool = DatabasePool::Sqlite(pool);
        let result = db_pool
            .execute_query("EXPLAIN ANALYZE DELETE FROM t", &[], 0, 100)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("EXPLAIN ANALYZE"));
    }

    #[tokio::test]
    async fn test_explain_select_not_blocked() {
        // EXPLAIN SELECT passes the security check (no ANALYZE + mutation).
        // SQLite can't subquery-wrap EXPLAIN output, so it may fail at execution,
        // but the error must NOT be our security rejection.
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("CREATE TABLE t (id INTEGER)")
            .execute(&pool)
            .await
            .unwrap();
        let db_pool = DatabasePool::Sqlite(pool);
        let result = db_pool
            .execute_query("EXPLAIN SELECT * FROM t", &[], 0, 100)
            .await;
        if let Err(ref e) = result {
            assert!(
                !e.contains("EXPLAIN ANALYZE"),
                "Should not be blocked by security check"
            );
            assert!(
                !e.contains("Multi-statement"),
                "Should not be blocked by multi-statement check"
            );
        }
    }

    #[tokio::test]
    async fn test_explain_analyze_select_not_blocked() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("CREATE TABLE t (id INTEGER)")
            .execute(&pool)
            .await
            .unwrap();
        let db_pool = DatabasePool::Sqlite(pool);
        let result = db_pool
            .execute_query("EXPLAIN ANALYZE SELECT * FROM t", &[], 0, 100)
            .await;
        // May fail at execution level, but must NOT be our security rejection
        if let Err(ref e) = result {
            assert!(
                !e.contains("mutation"),
                "SELECT should not be blocked as mutation"
            );
        }
    }

    #[tokio::test]
    async fn test_pragma_write_treated_as_mutation() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let db_pool = DatabasePool::Sqlite(pool);
        let result = db_pool
            .execute_query("PRAGMA journal_mode=WAL", &[], 0, 100)
            .await
            .unwrap();
        assert!(!result.is_select);
    }

    #[tokio::test]
    async fn test_pragma_read_not_blocked() {
        // PRAGMA without = should be classified as SELECT.
        // Subquery wrapping may fail in SQLite, but security check must pass.
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let db_pool = DatabasePool::Sqlite(pool);
        let result = db_pool
            .execute_query("PRAGMA table_info('sqlite_master')", &[], 0, 100)
            .await;
        if let Err(ref e) = result {
            assert!(!e.contains("Multi-statement"), "Should not be blocked");
        }
    }

    // normalize_placeholders / count_placeholders unit tests moved
    // to `db::sql_scanner::tests` along with the implementations.

    #[tokio::test]
    async fn test_multi_statement_rejected_in_execute() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("CREATE TABLE t (id INTEGER)")
            .execute(&pool)
            .await
            .unwrap();
        let db_pool = DatabasePool::Sqlite(pool);
        let result = db_pool
            .execute_query("SELECT 1; DROP TABLE t", &[], 0, 100)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Multi-statement"));
    }

    // --- Phase 4: Bind parameter safety tests ---

    #[test]
    fn test_validate_bind_rejects_array() {
        let vals = vec![serde_json::json!(1), serde_json::json!([2, 3])];
        let result = validate_bind_values(&vals);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("array"));
    }

    #[test]
    fn test_validate_bind_rejects_object() {
        let vals = vec![serde_json::json!({"key": "val"})];
        let result = validate_bind_values(&vals);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("object"));
    }

    #[test]
    fn test_validate_bind_accepts_primitives() {
        let vals = vec![
            serde_json::Value::Null,
            serde_json::json!(true),
            serde_json::json!(42),
            serde_json::json!(2.5),
            serde_json::json!("hello"),
        ];
        assert!(validate_bind_values(&vals).is_ok());
    }

    #[tokio::test]
    async fn test_bind_count_mismatch_too_few() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("CREATE TABLE t (a INTEGER, b INTEGER)")
            .execute(&pool)
            .await
            .unwrap();
        let db_pool = DatabasePool::Sqlite(pool);
        let result = db_pool
            .execute_query(
                "SELECT * FROM t WHERE a = ? AND b = ?",
                &[serde_json::json!(1)],
                0,
                100,
            )
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not match"));
    }

    #[tokio::test]
    async fn test_bind_count_mismatch_too_many() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("CREATE TABLE t (a INTEGER)")
            .execute(&pool)
            .await
            .unwrap();
        let db_pool = DatabasePool::Sqlite(pool);
        let result = db_pool
            .execute_query(
                "SELECT * FROM t WHERE a = ?",
                &[
                    serde_json::json!(1),
                    serde_json::json!(2),
                    serde_json::json!(3),
                ],
                0,
                100,
            )
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not match"));
    }

    #[tokio::test]
    async fn test_bind_count_matches() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("CREATE TABLE t (a INTEGER, b TEXT)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO t VALUES (1, 'x'), (2, 'y')")
            .execute(&pool)
            .await
            .unwrap();
        let db_pool = DatabasePool::Sqlite(pool);
        let result = db_pool
            .execute_query(
                "SELECT * FROM t WHERE a = ? AND b = ?",
                &[serde_json::json!(1), serde_json::json!("x")],
                0,
                100,
            )
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().rows.len(), 1);
    }

    // ─────── Fail-secure: keychain unavailable must NOT fall back to plaintext ───────

    #[tokio::test]
    // The std::Mutex guard intentionally spans the awaits below: the lock
    // serializes tests that mutate the keychain's global mock-failure flag,
    // and dropping it earlier would let parallel tests corrupt that state.
    // Tests run on a single tokio thread per case, so blocking is fine.
    #[allow(clippy::await_holding_lock)]
    async fn create_connection_fails_secure_when_keychain_unavailable() {
        // Epic 16, Story 02 invariant: if `store_secret` fails, the
        // connection row must NOT be inserted with a plaintext password.
        // We force the keychain to error and verify both the Err return
        // AND that no row leaked into the database.
        use crate::db::keychain::{force_keychain_failure, KEYCHAIN_TEST_LOCK};

        let _guard = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let pool = setup_test_pool().await;

        force_keychain_failure(true);
        let result = create_connection(
            &pool,
            CreateConnection {
                name: "fail-secure-test".to_string(),
                driver: "postgres".to_string(),
                host: Some("localhost".to_string()),
                port: Some(5432),
                database_name: Some("db".to_string()),
                username: Some("user".to_string()),
                password: Some("very-sensitive-password".to_string()),
                ssl_mode: None,
                timeout_ms: None,
                query_timeout_ms: None,
                ttl_seconds: None,
                max_pool_size: None,
                is_readonly: None,
            },
        )
        .await;
        force_keychain_failure(false);

        // 1. The call must fail loudly.
        let err = result.expect_err("create_connection must error when keychain fails");
        assert!(
            err.to_lowercase().contains("password") || err.to_lowercase().contains("keychain"),
            "error should mention password/keychain, got: {err}"
        );

        // 2. No row must exist in the database — that's the actual fail-
        // secure invariant. A leaked row with plaintext password would
        // be the worst case (silent regression of T15 from epic 16).
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM connections")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            count, 0,
            "no connection row may exist when keychain storage failed",
        );

        // 3. Sanity: when the keychain works, the same call must succeed.
        // Guards against the test always passing for the wrong reason.
        let ok = create_connection(
            &pool,
            CreateConnection {
                name: "fail-secure-baseline".to_string(),
                driver: "postgres".to_string(),
                host: Some("localhost".to_string()),
                port: Some(5432),
                database_name: Some("db".to_string()),
                username: Some("user".to_string()),
                password: Some("baseline-password".to_string()),
                ssl_mode: None,
                timeout_ms: None,
                query_timeout_ms: None,
                ttl_seconds: None,
                max_pool_size: None,
                is_readonly: None,
            },
        )
        .await;
        // The baseline only succeeds if the test environment has a working
        // keyring backend (macOS Keychain, etc). In headless CI this can
        // legitimately fail — accept either Ok or Err here. The fail-secure
        // case above is what we really care about.
        let _ = ok;
    }

    // ─────── Cache invalidation: update_connection → next query uses new pool ───────

    #[tokio::test]
    async fn update_connection_followed_by_invalidate_yields_fresh_pool() {
        // Epic 16, Story 05 invariant (L102): the Tauri command sequence
        // (update_connection → conn_manager.invalidate) must guarantee
        // that the next `get_pool` call returns a pool built from the
        // updated row, not a stale cached one.
        //
        // We can't test the full Tauri command from here, but we can
        // assert the invariant on the `PoolManager` it ultimately calls:
        // (1) without invalidate, the cache survives an update; (2) after
        // invalidate, the next get_pool creates a fresh Arc whose
        // PoolEntry reflects the new config. This is the bug the L102
        // task was designed to catch — a regression where invalidate
        // stops being called would leak stale config.
        let pool = setup_test_pool().await;
        let manager = std::sync::Arc::new(PoolManager::new_standalone(
            super::super::lookup::SqliteLookup::new(pool.clone()),
            pool.clone(),
        ));

        let created = create_connection(
            &pool,
            CreateConnection {
                name: "pool-invalidate-test".to_string(),
                driver: "sqlite".to_string(),
                host: None,
                port: None,
                database_name: Some(":memory:".to_string()),
                username: None,
                password: None,
                ssl_mode: None,
                timeout_ms: Some(5000),
                query_timeout_ms: Some(10000),
                ttl_seconds: Some(300),
                max_pool_size: Some(2),
                is_readonly: None,
            },
        )
        .await
        .expect("create_connection (sqlite) must succeed");

        let id = created.id.clone();

        // First get_pool: pool is created and cached with timeout=10000.
        let pool_a = manager.get_pool(&id).await.expect("get_pool must succeed");
        assert_eq!(manager.get_query_timeout(&id).await, Some(10000));

        // Update the row's query_timeout_ms in the database.
        update_connection(
            &pool,
            &id,
            UpdateConnection {
                name: None,
                driver: None,
                host: None,
                port: None,
                database_name: None,
                username: None,
                password: None,
                ssl_mode: None,
                timeout_ms: None,
                query_timeout_ms: Some(99000),
                ttl_seconds: None,
                max_pool_size: None,
                is_readonly: None,
            },
        )
        .await
        .expect("update_connection must succeed");

        // Without invalidate, the cache is still serving the old timeout.
        // This documents the (intentional) fact that PoolManager has no
        // automatic awareness of DB row changes — it relies on the
        // caller (the Tauri command) to invalidate.
        assert_eq!(
            manager.get_query_timeout(&id).await,
            Some(10000),
            "cache must survive an update with no invalidate (documents the invariant)",
        );

        // Tauri command's invalidate step.
        manager.invalidate(&id).await;

        // Next get_pool builds a fresh pool from the current DB row.
        let pool_b = manager
            .get_pool(&id)
            .await
            .expect("get_pool after invalidate must succeed");

        assert!(
            !std::sync::Arc::ptr_eq(&pool_a, &pool_b),
            "pool Arc must differ — cache was rebuilt, not reused",
        );
        assert_eq!(
            manager.get_query_timeout(&id).await,
            Some(99000),
            "rebuilt PoolEntry must reflect the updated row's query_timeout_ms",
        );
    }
}
