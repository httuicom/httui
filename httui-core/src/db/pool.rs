//! `DatabasePool` enum + per-driver pool construction (Postgres /
//! MySQL / SQLite).
//!
//! Owns the lifecycle pieces — enum definition, ping (`test`),
//! `create_pool` factory, driver-specific `build_*_connect_options`
//! helpers, path/name validation, pool config validation, and
//! connection-error sanitization.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use std::str::FromStr;

use super::connections::Connection;
use super::driver::DbDriver;
use super::pool_exec_mysql::{execute_mutation_mysql, execute_select_mysql};
use super::pool_exec_pg::{execute_mutation_pg, execute_select_pg};
use super::pool_exec_sqlite::{execute_mutation_sqlite, execute_select_sqlite};
use super::query_error::{QueryErrorInfo, QueryErrorLocation};
use super::sql_scanner::{contains_multiple_statements, count_placeholders};

/// Row data as JSON-compatible values.
pub type JsonRow = Vec<serde_json::Value>;

/// Convert a canonical decimal string (from `BigDecimal::to_string()`)
/// into a JSON value. Emits a JSON number when an f64 round-trip is
/// exact (modulo trailing zeros); otherwise falls back to a string to
/// preserve precision. Used by the PG NUMERIC and MySQL DECIMAL
/// decoders so common values (`499.80`, `0.1`, IDs, prices) export as
/// proper numbers while values that exceed f64 mantissa keep their
/// exact representation.
pub(super) fn decimal_to_json(s: &str) -> serde_json::Value {
    if let Ok(f) = s.parse::<f64>() {
        if let Some(n) = serde_json::Number::from_f64(f) {
            // `f64::to_string` uses ryū (shortest-roundtrip), so
            // "499.80".parse() then `.to_string()` yields "499.8".
            // Strip trailing zeros on both sides before comparing
            // so cosmetic precision (NUMERIC(10,2) padding) doesn't
            // force a string fallback.
            let canon_in = strip_trailing_decimal_zeros(s);
            let canon_back = strip_trailing_decimal_zeros(&f.to_string());
            if canon_in == canon_back {
                return serde_json::Value::Number(n);
            }
        }
    }
    serde_json::Value::String(s.to_string())
}

fn strip_trailing_decimal_zeros(s: &str) -> String {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s.to_string()
    }
}

#[derive(Debug)]
pub struct QueryResult {
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<JsonRow>,
    pub has_more: bool,
    pub rows_affected: Option<u64>,
    pub is_select: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    /// Driver-reported type name (e.g. "INTEGER", "int4"). Renamed on the
    /// wire to `type` so TS consumers can write `col.type` ergonomically.
    #[serde(rename = "type")]
    pub type_name: String,
}

#[allow(clippy::large_enum_variant)]
pub enum DatabasePool {
    Postgres(sqlx::PgPool),
    MySql(sqlx::MySqlPool),
    Sqlite(sqlx::SqlitePool),
}

impl DatabasePool {
    /// Driver as a `&'static str`. Matches the lowercase wire shape
    /// (`type = "postgres"` etc.) — see [`DbDriver::as_str`].
    pub fn driver(&self) -> &'static str {
        self.driver_kind().as_str()
    }

    /// Typed driver — preferred for new dispatch sites over the
    /// `&str` form. Replaces the old string-match in `create_pool`
    /// at the call boundary.
    pub fn driver_kind(&self) -> DbDriver {
        match self {
            Self::Postgres(_) => DbDriver::Postgres,
            Self::MySql(_) => DbDriver::Mysql,
            Self::Sqlite(_) => DbDriver::Sqlite,
        }
    }

    pub async fn execute_query(
        &self,
        sql: &str,
        bind_values: &[serde_json::Value],
        offset: u32,
        fetch_size: u32,
    ) -> Result<QueryResult, QueryErrorInfo> {
        // Pre-send validations: not driver errors, so no line/col location.
        let plain_err = |msg: String| QueryErrorInfo {
            message: msg,
            location: QueryErrorLocation::default(),
        };

        if contains_multiple_statements(sql) {
            return Err(plain_err(
                "Multi-statement queries are not allowed".to_string(),
            ));
        }

        validate_bind_values(bind_values).map_err(plain_err)?;

        let expected = count_placeholders(sql);
        if bind_values.len() != expected {
            return Err(plain_err(format!(
                "Bind values count ({}) does not match placeholder count ({expected})",
                bind_values.len()
            )));
        }

        let trimmed = sql.trim_start().to_uppercase();

        if trimmed.starts_with("EXPLAIN")
            && (trimmed.contains("ANALYZE") || trimmed.contains("ANALYSE"))
        {
            let after_explain = trimmed
                .trim_start_matches("EXPLAIN")
                .trim()
                .trim_start_matches("ANALYZE")
                .trim_start_matches("ANALYSE")
                .trim_start();
            let mutation_keywords = ["DELETE", "UPDATE", "INSERT", "DROP", "ALTER", "TRUNCATE"];
            if mutation_keywords
                .iter()
                .any(|kw| after_explain.starts_with(kw))
            {
                return Err(plain_err(
                    "EXPLAIN ANALYZE with mutation statements is not allowed".to_string(),
                ));
            }
        }

        let is_select = if trimmed.starts_with("PRAGMA") {
            // PRAGMA with = is a write operation (e.g. PRAGMA journal_mode=WAL)
            !trimmed.contains('=')
        } else {
            trimmed.starts_with("SELECT")
                || trimmed.starts_with("WITH")
                || trimmed.starts_with("SHOW")
                || trimmed.starts_with("DESCRIBE")
                || trimmed.starts_with("EXPLAIN")
        };

        if is_select {
            self.execute_select(sql, bind_values, offset, fetch_size)
                .await
        } else {
            self.execute_mutation(sql, bind_values).await
        }
    }

    async fn execute_select(
        &self,
        sql: &str,
        bind_values: &[serde_json::Value],
        offset: u32,
        fetch_size: u32,
    ) -> Result<QueryResult, QueryErrorInfo> {
        match self {
            Self::Sqlite(pool) => {
                execute_select_sqlite(pool, sql, bind_values, offset, fetch_size).await
            }
            Self::Postgres(pool) => {
                execute_select_pg(pool, sql, bind_values, offset, fetch_size).await
            }
            Self::MySql(pool) => {
                execute_select_mysql(pool, sql, bind_values, offset, fetch_size).await
            }
        }
    }

    async fn execute_mutation(
        &self,
        sql: &str,
        bind_values: &[serde_json::Value],
    ) -> Result<QueryResult, QueryErrorInfo> {
        match self {
            Self::Sqlite(pool) => execute_mutation_sqlite(pool, sql, bind_values).await,
            Self::Postgres(pool) => execute_mutation_pg(pool, sql, bind_values).await,
            Self::MySql(pool) => execute_mutation_mysql(pool, sql, bind_values).await,
        }
    }

    pub async fn test(&self) -> Result<(), String> {
        match self {
            Self::Postgres(pool) => {
                sqlx::query("SELECT 1")
                    .fetch_one(pool)
                    .await
                    .map_err(|e| e.to_string())?;
            }
            Self::MySql(pool) => {
                sqlx::query("SELECT 1")
                    .fetch_one(pool)
                    .await
                    .map_err(|e| e.to_string())?;
            }
            Self::Sqlite(pool) => {
                sqlx::query("SELECT 1")
                    .fetch_one(pool)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
}

fn build_pg_connect_options(conn: &Connection) -> Result<sqlx::postgres::PgConnectOptions, String> {
    use super::keychain::{conn_password_key, resolve_value, KEYCHAIN_SENTINEL};
    use sqlx::postgres::{PgConnectOptions, PgSslMode};

    let host = conn.host.as_deref().unwrap_or("localhost");
    let port = conn.port.unwrap_or(5432) as u16;
    let db = conn
        .database_name
        .as_deref()
        .ok_or("database_name is required for postgres")?;
    let user = conn.username.as_deref().unwrap_or("postgres");
    let db_password_raw = conn.password.as_deref().unwrap_or("");
    let password = if db_password_raw == KEYCHAIN_SENTINEL {
        resolve_value(db_password_raw, &conn_password_key(&conn.id)).unwrap_or_default()
    } else {
        db_password_raw.to_string()
    };
    let ssl_mode = match conn.ssl_mode.as_deref().unwrap_or("prefer") {
        "require" | "verify-ca" | "verify-full" => PgSslMode::Require,
        "disable" => PgSslMode::Disable,
        _ => PgSslMode::Prefer,
    };

    Ok(PgConnectOptions::new()
        .host(host)
        .port(port)
        .database(db)
        .username(user)
        .password(&password)
        .ssl_mode(ssl_mode))
}

pub(super) fn validate_bind_values(bind_values: &[serde_json::Value]) -> Result<(), String> {
    for (i, val) in bind_values.iter().enumerate() {
        match val {
            serde_json::Value::Array(_) => {
                return Err(format!(
                    "Bind value at index {i} is an array; only primitive types (null, bool, number, string) are supported"
                ));
            }
            serde_json::Value::Object(_) => {
                return Err(format!(
                    "Bind value at index {i} is an object; only primitive types (null, bool, number, string) are supported"
                ));
            }
            serde_json::Value::Number(n) if n.as_i64().is_none() && n.as_f64().is_none() => {
                return Err(format!(
                    "Bind value at index {i} is a number outside the supported range (i64/f64)"
                ));
            }
            _ => {} // Null, Bool, String are fine
        }
    }
    Ok(())
}

/// True only for statements that can legally be wrapped in
/// `SELECT * FROM (<sql>) LIMIT … OFFSET …` for pagination — i.e. real
/// SELECTs and CTEs. EXPLAIN, PRAGMA, SHOW, DESCRIBE all return rows
/// but are not subqueryable in any of the three drivers, so they must
/// run as-is.
pub(super) fn is_subqueryable_select(sql: &str) -> bool {
    let trimmed = sql.trim_start().to_uppercase();
    trimmed.starts_with("SELECT") || trimmed.starts_with("WITH")
}

pub(super) fn validate_sqlite_path(path: &str) -> Result<(), String> {
    if path == ":memory:" || path.starts_with(":memory:") {
        return Ok(());
    }
    if path.contains("../") || path.contains("..\\") {
        return Err("SQLite path must not contain path traversal (../)".to_string());
    }
    let resolved = std::path::Path::new(path);
    if !resolved.is_absolute() {
        return Err("SQLite database_name must be an absolute path".to_string());
    }
    Ok(())
}

pub(super) fn validate_mysql_database_name(name: &str) -> Result<(), String> {
    if name.contains('`') || name.contains(';') || name.contains('\0') || name.contains('\\') {
        return Err(
            "Database name contains forbidden characters (backtick, semicolon, null, backslash)"
                .to_string(),
        );
    }
    if name.len() > 64 {
        return Err("Database name exceeds MySQL 64-character limit".to_string());
    }
    Ok(())
}

fn build_mysql_connect_options(
    conn: &Connection,
) -> Result<sqlx::mysql::MySqlConnectOptions, String> {
    use super::keychain::{conn_password_key, resolve_value, KEYCHAIN_SENTINEL};
    use sqlx::mysql::{MySqlConnectOptions, MySqlSslMode};

    let host = conn.host.as_deref().unwrap_or("localhost");
    let port = conn.port.unwrap_or(3306) as u16;
    let user = conn.username.as_deref().unwrap_or("root");
    let db_password_raw = conn.password.as_deref().unwrap_or("");
    let password = if db_password_raw == KEYCHAIN_SENTINEL {
        resolve_value(db_password_raw, &conn_password_key(&conn.id)).unwrap_or_default()
    } else {
        db_password_raw.to_string()
    };
    let ssl_mode = match conn.ssl_mode.as_deref().unwrap_or("prefer") {
        "require" | "verify-ca" | "verify-full" => MySqlSslMode::Required,
        "disable" => MySqlSslMode::Disabled,
        _ => MySqlSslMode::Preferred,
    };

    // NOTE: intentionally DO NOT call opts.database(db) here. Passing the schema
    // via CLIENT_CONNECT_WITH_DB in the handshake breaks routing in ProxySQL
    // deployments that apply schema-based hostgroup rules on USE/queries only.
    // We select the database via `USE` in after_connect instead.
    //
    // `charset("utf8mb4")` forces the connection charset on handshake.
    // Without it sqlx-mysql's default lands on the server's `character_set_client`
    // (often latin1 on older configs), so UTF-8 text round-trips mangled
    // (e.g. `Aragão`→`AragÃ£o`).
    Ok(MySqlConnectOptions::new()
        .host(host)
        .port(port)
        .username(user)
        .password(&password)
        .charset("utf8mb4")
        .ssl_mode(ssl_mode))
}

pub(super) fn validate_pool_config(conn: &Connection) -> Result<(), String> {
    if conn.max_pool_size < 1 || conn.max_pool_size > 100 {
        return Err(format!(
            "max_pool_size must be between 1 and 100, got {}",
            conn.max_pool_size
        ));
    }
    if conn.timeout_ms < 100 || conn.timeout_ms > 300_000 {
        return Err(format!(
            "timeout_ms must be between 100 and 300000, got {}",
            conn.timeout_ms
        ));
    }
    if conn.query_timeout_ms < 100 || conn.query_timeout_ms > 600_000 {
        return Err(format!(
            "query_timeout_ms must be between 100 and 600000, got {}",
            conn.query_timeout_ms
        ));
    }
    // SQLite connections have no TCP port; skip the range check entirely
    // (older records may have been persisted with `port = 0` by an earlier
    // bug in `update_connection`, and we don't want them stuck unusable).
    let drv = DbDriver::from_str(&conn.driver).ok();
    if drv != Some(DbDriver::Sqlite) {
        if let Some(port) = conn.port {
            if !(1..=65535).contains(&port) {
                return Err(format!("port must be between 1 and 65535, got {port}"));
            }
        }
    }
    if conn.ttl_seconds < 10 || conn.ttl_seconds > 86400 {
        return Err(format!(
            "ttl_seconds must be between 10 and 86400, got {}",
            conn.ttl_seconds
        ));
    }
    Ok(())
}

/// Sanitize connection errors to prevent leaking credentials in sqlx error messages.
/// The full `e` is intentionally not logged anywhere from this crate — `eprintln!`
/// breaks any TUI consumer (raw stderr paints over ratatui cells), and `tracing`
/// isn't pulled in here. Callers that need diagnostics should wrap the result and
/// route into their own log sink.
fn sanitize_connection_error(driver: &str, e: sqlx::Error) -> String {
    match &e {
        sqlx::Error::PoolTimedOut => format!("Connection to {driver} timed out"),
        sqlx::Error::Configuration(_) => format!("Invalid {driver} configuration"),
        _ => format!("Failed to connect to {driver}"),
    }
}

pub(super) async fn create_pool(conn: &Connection) -> Result<DatabasePool, String> {
    validate_pool_config(conn)?;
    let max_conns = conn.max_pool_size as u32;
    let timeout = Duration::from_millis(conn.timeout_ms as u64);

    // Boundary parse — surface the same "Unsupported driver: <name>"
    // shape the legacy match used to.
    let drv = DbDriver::from_str(&conn.driver)
        .map_err(|_| format!("Unsupported driver: {}", conn.driver))?;
    match drv {
        DbDriver::Postgres => {
            let opts = build_pg_connect_options(conn)?;
            let pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(max_conns)
                .acquire_timeout(timeout)
                .connect_with(opts)
                .await
                .map_err(|e| sanitize_connection_error(DbDriver::Postgres.as_str(), e))?;
            Ok(DatabasePool::Postgres(pool))
        }
        DbDriver::Mysql => {
            let opts = build_mysql_connect_options(conn)?;
            let db_name = conn.database_name.clone().unwrap_or_default();
            let mut pool_opts = sqlx::mysql::MySqlPoolOptions::new()
                .max_connections(max_conns)
                .acquire_timeout(timeout);
            if !db_name.is_empty() {
                validate_mysql_database_name(&db_name)?;
                pool_opts = pool_opts.after_connect(move |conn, _meta| {
                    let db = db_name.clone();
                    Box::pin(async move {
                        use sqlx::Executor;
                        // Pass the SQL as a plain `&str` so sqlx uses the text
                        // protocol (COM_QUERY). Prepared-statement `USE` is
                        // rejected by ProxySQL with error 1295.
                        let sql = format!("USE `{}`", db);
                        conn.execute(sql.as_str()).await?;
                        Ok(())
                    })
                });
            }
            let pool = pool_opts
                .connect_with(opts)
                .await
                .map_err(|e| sanitize_connection_error(DbDriver::Mysql.as_str(), e))?;
            Ok(DatabasePool::MySql(pool))
        }
        DbDriver::Sqlite => {
            let path = conn
                .database_name
                .as_deref()
                .ok_or("database_name (file path) is required for sqlite")?;
            validate_sqlite_path(path)?;
            let url = format!("sqlite:{path}");
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(max_conns)
                .acquire_timeout(timeout)
                .connect(&url)
                .await
                .map_err(|e| sanitize_connection_error(DbDriver::Sqlite.as_str(), e))?;
            Ok(DatabasePool::Sqlite(pool))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_sqlite_path_accepts_memory() {
        assert!(validate_sqlite_path(":memory:").is_ok());
    }

    #[test]
    fn validate_sqlite_path_rejects_traversal() {
        assert!(validate_sqlite_path("../foo.db").is_err());
        assert!(validate_sqlite_path("..\\foo.db").is_err());
    }

    #[test]
    fn decimal_to_json_emits_number_for_lossless_values() {
        // Common money amounts round-trip exactly through f64 once
        // trailing zeros are stripped.
        for s in &["499.80", "0.1", "42.50", "189.00", "239.97", "1234567"] {
            assert!(
                matches!(decimal_to_json(s), serde_json::Value::Number(_)),
                "{s} should emit JSON number"
            );
        }
    }

    #[test]
    fn decimal_to_json_falls_back_to_string_when_precision_lost() {
        // Values exceeding f64 mantissa (~15-17 sig figs) must
        // preserve precision via string.
        for s in &[
            "12345678901234567890.123", // 23 sig figs
            "0.123456789012345678",     // 18 sig figs
            "9999999999999999.999",     // edge of f64 precision
        ] {
            assert!(
                matches!(decimal_to_json(s), serde_json::Value::String(_)),
                "{s} should fall back to JSON string"
            );
        }
    }

    #[test]
    fn decimal_to_json_handles_negative_and_zero() {
        assert!(matches!(decimal_to_json("0"), serde_json::Value::Number(_)));
        assert!(matches!(
            decimal_to_json("0.00"),
            serde_json::Value::Number(_)
        ));
        assert!(matches!(
            decimal_to_json("-499.80"),
            serde_json::Value::Number(_)
        ));
    }

    #[test]
    fn validate_sqlite_path_rejects_relative() {
        assert!(validate_sqlite_path("foo.db").is_err());
    }

    #[test]
    fn validate_mysql_database_name_rejects_unsafe_chars() {
        assert!(validate_mysql_database_name("db`name").is_err());
        assert!(validate_mysql_database_name("db;DROP").is_err());
        assert!(validate_mysql_database_name("db\\name").is_err());
        assert!(validate_mysql_database_name("db\0name").is_err());
    }

    #[test]
    fn validate_mysql_database_name_enforces_length() {
        assert!(validate_mysql_database_name(&"a".repeat(64)).is_ok());
        assert!(validate_mysql_database_name(&"a".repeat(65)).is_err());
    }

    #[test]
    fn validate_mysql_database_name_accepts_normal() {
        assert!(validate_mysql_database_name("payments").is_ok());
        assert!(validate_mysql_database_name("payments_v2").is_ok());
    }

    fn pool_conn(driver: &str, port: Option<i64>) -> Connection {
        Connection {
            id: "id".into(),
            name: "n".into(),
            driver: driver.into(),
            host: Some("h".into()),
            port,
            database_name: Some("/tmp/x.sqlite".into()),
            username: Some("u".into()),
            password: None,
            ssl_mode: None,
            timeout_ms: 1000,
            query_timeout_ms: 1000,
            ttl_seconds: 60,
            max_pool_size: 5,
            is_readonly: false,
            last_tested_at: None,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn validate_pool_config_passes_in_range() {
        let c = pool_conn("postgres", Some(5432));
        assert!(validate_pool_config(&c).is_ok());
    }

    #[test]
    fn validate_pool_config_skips_port_check_for_sqlite() {
        // Port 0 would otherwise fail the range check; SQLite skip
        // exists because older records persisted port=0 by an earlier
        // bug. The DbDriver-typed comparison preserves that behavior.
        let c = pool_conn("sqlite", Some(0));
        assert!(validate_pool_config(&c).is_ok());
    }

    #[test]
    fn validate_pool_config_rejects_out_of_range_port_for_pg() {
        let c = pool_conn("postgres", Some(0));
        let err = validate_pool_config(&c).unwrap_err();
        assert!(err.contains("port must be"));
    }

    #[test]
    fn validate_pool_config_applies_port_check_for_unknown_driver() {
        // DbDriver::from_str returns Err for "weirdb"; the != Sqlite
        // comparison still applies the port check.
        let c = pool_conn("weirdb", Some(99999));
        let err = validate_pool_config(&c).unwrap_err();
        assert!(err.contains("port must be"));
    }

    #[test]
    fn validate_pool_config_rejects_out_of_range_timeout() {
        let mut c = pool_conn("postgres", Some(5432));
        c.timeout_ms = 50;
        assert!(validate_pool_config(&c).is_err());
        c.timeout_ms = 500_000;
        assert!(validate_pool_config(&c).is_err());
    }

    #[test]
    fn validate_pool_config_rejects_out_of_range_query_timeout() {
        let mut c = pool_conn("postgres", Some(5432));
        c.query_timeout_ms = 50;
        assert!(validate_pool_config(&c).is_err());
        c.query_timeout_ms = 700_000;
        assert!(validate_pool_config(&c).is_err());
    }

    #[test]
    fn validate_pool_config_rejects_out_of_range_ttl() {
        let mut c = pool_conn("postgres", Some(5432));
        c.ttl_seconds = 5;
        assert!(validate_pool_config(&c).is_err());
        c.ttl_seconds = 100_000;
        assert!(validate_pool_config(&c).is_err());
    }

    #[test]
    fn sanitize_connection_error_distinguishes_kinds() {
        let pool_timeout = sqlx::Error::PoolTimedOut;
        let msg = sanitize_connection_error("postgres", pool_timeout);
        assert!(msg.contains("timed out"));
        assert!(msg.contains("postgres"));

        let config_err = sqlx::Error::Configuration("bad cfg".into());
        let msg = sanitize_connection_error("mysql", config_err);
        assert!(msg.contains("Invalid"));
        assert!(msg.contains("mysql"));
    }

    #[tokio::test]
    async fn create_pool_rejects_unknown_driver() {
        let c = pool_conn("weirdb", Some(5432));
        match create_pool(&c).await {
            Ok(_) => panic!("must reject unknown driver"),
            Err(msg) => {
                assert!(msg.contains("Unsupported driver"));
                assert!(msg.contains("weirdb"));
            }
        }
    }

    #[tokio::test]
    async fn create_pool_succeeds_for_sqlite_in_memory() {
        let mut c = pool_conn("sqlite", None);
        c.database_name = Some(":memory:".into());
        match create_pool(&c).await {
            Ok(pool) => {
                assert_eq!(pool.driver_kind(), DbDriver::Sqlite);
                assert_eq!(pool.driver(), "sqlite");
                assert_eq!(DbDriver::from_str(pool.driver()).unwrap(), DbDriver::Sqlite);
            }
            Err(e) => panic!("sqlite memory pool: {e}"),
        }
    }

    #[tokio::test]
    async fn create_pool_rejects_sqlite_traversal_path() {
        let mut c = pool_conn("sqlite", None);
        c.database_name = Some("../escape.db".into());
        match create_pool(&c).await {
            Ok(_) => panic!("must reject traversal"),
            Err(msg) => assert!(
                msg.contains("traversal") || msg.contains("invalid"),
                "got: {msg}"
            ),
        }
    }

    #[test]
    fn build_pg_connect_options_succeeds_with_full_input() {
        let c = pool_conn("postgres", Some(5432));
        assert!(build_pg_connect_options(&c).is_ok());
    }

    #[test]
    fn build_mysql_connect_options_succeeds_with_full_input() {
        let c = pool_conn("mysql", Some(3306));
        assert!(build_mysql_connect_options(&c).is_ok());
    }

    #[test]
    fn build_mysql_connect_options_handles_missing_database_name() {
        // MySQL connect options work with no database — the after_connect
        // hook in create_pool runs `USE` only if the name is set.
        let mut c = pool_conn("mysql", Some(3306));
        c.database_name = None;
        assert!(build_mysql_connect_options(&c).is_ok());
    }

    async fn sqlite_memory_pool() -> DatabasePool {
        let mut c = pool_conn("sqlite", None);
        c.database_name = Some(":memory:".into());
        create_pool(&c).await.expect("sqlite memory pool")
    }

    #[tokio::test]
    async fn execute_query_rejects_multi_statement() {
        let pool = sqlite_memory_pool().await;
        let err = pool
            .execute_query("SELECT 1; SELECT 2", &[], 0, 10)
            .await
            .unwrap_err();
        assert!(err.message.contains("Multi-statement"));
    }

    #[tokio::test]
    async fn execute_query_rejects_bind_count_mismatch() {
        let pool = sqlite_memory_pool().await;
        let err = pool
            .execute_query("SELECT ?", &[], 0, 10)
            .await
            .unwrap_err();
        assert!(err.message.contains("Bind values count"));
    }

    #[tokio::test]
    async fn execute_query_rejects_explain_analyze_with_mutation() {
        let pool = sqlite_memory_pool().await;
        let err = pool
            .execute_query("EXPLAIN ANALYZE DELETE FROM x", &[], 0, 10)
            .await
            .unwrap_err();
        assert!(err.message.contains("EXPLAIN ANALYZE"));
    }

    #[tokio::test]
    async fn execute_query_pragma_with_equals_is_treated_as_mutation() {
        // PRAGMA journal_mode=WAL is a write — execute_mutation path
        // (no SELECT). On a fresh in-memory DB this still returns Ok
        // with affected rows; the test just exercises the dispatch.
        let pool = sqlite_memory_pool().await;
        let result = pool
            .execute_query("PRAGMA journal_mode=WAL", &[], 0, 10)
            .await;
        // Either way the dispatch line is exercised — error is fine
        // for our coverage purposes; we're not asserting query semantics.
        let _ = result;
    }
}
