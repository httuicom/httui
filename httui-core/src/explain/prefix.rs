//! `EXPLAIN`-prefix builder for SQL drivers.
//!
//! When a SQL block runs with `explain=true` in its info-string, the
//! consumer (DB executor) swaps the user's SQL for an `EXPLAIN`-prefixed
//! variant before dispatch — Postgres `EXPLAIN (ANALYZE, BUFFERS, FORMAT
//! JSON)`, MySQL `EXPLAIN FORMAT=JSON`. The result rows then feed
//! `crate::explain::parse_postgres_explain` / `parse_mysql_explain`.
//!
//! This module is intentionally tiny: just the SQL-string transform +
//! a body cap. Driver dispatch + result extraction live closer to
//! the executor.
//!
//! SQLite / BigQuery / Snowflake are explicitly unsupported per
//! spec ("best-effort or NOT supported with clear message").
//! MongoDB is excluded too — it doesn't take SQL; the Mongo executor
//! handles `db.collection.explain("executionStats")` separately.

use std::fmt;

/// Body cap for stored EXPLAIN payloads. Per spec: "Body cap
/// 200 KB; truncate marker if exceeded".
pub const EXPLAIN_BODY_CAP: usize = 200_000;

const TRUNCATE_MARKER: &str = "\n[explain payload truncated]";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExplainError {
    /// The driver string didn't map to a SQL driver we know how to
    /// `EXPLAIN`. Carries the driver string verbatim so the consumer
    /// can show "EXPLAIN unavailable for this driver" with the
    /// actual name.
    Unsupported { driver: String },
}

impl fmt::Display for ExplainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExplainError::Unsupported { driver } => {
                write!(f, "EXPLAIN unavailable for driver `{driver}`")
            }
        }
    }
}

impl std::error::Error for ExplainError {}

/// Wrap `sql` with the driver-appropriate `EXPLAIN` prefix that
/// produces a JSON plan in row 0 of the result. Errors when the
/// driver is one we don't support EXPLAIN for.
pub fn prefix_explain_sql(driver: &str, sql: &str) -> Result<String, ExplainError> {
    let trimmed_sql = sql.trim_start_matches([';', ' ', '\t', '\n', '\r']);
    if trimmed_sql.is_empty() {
        // Empty input — caller's bug, not ours; bubble the unsupported
        // shape so the surface is uniform. (We could add a separate
        // EmptySql variant but that splits the failure surface.)
        return Err(ExplainError::Unsupported {
            driver: driver.to_string(),
        });
    }
    match normalize_driver(driver) {
        Some(SqlDriver::Postgres) => Ok(format!(
            "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) {trimmed_sql}",
        )),
        Some(SqlDriver::Mysql) => Ok(format!("EXPLAIN FORMAT=JSON {trimmed_sql}")),
        None => Err(ExplainError::Unsupported {
            driver: driver.to_string(),
        }),
    }
}

/// Truncate `body` if it exceeds [`EXPLAIN_BODY_CAP`]. Returns the
/// (possibly truncated) string + a flag indicating whether truncation
/// happened — the consumer can surface it to the user ("plan
/// truncated; full payload not stored").
pub fn cap_explain_body(body: &str) -> (String, bool) {
    if body.len() <= EXPLAIN_BODY_CAP {
        return (body.to_string(), false);
    }
    let keep = EXPLAIN_BODY_CAP.saturating_sub(TRUNCATE_MARKER.len());
    let mut s = String::with_capacity(EXPLAIN_BODY_CAP);
    // Walk the boundary to a valid char index so we never split a
    // multi-byte UTF-8 sequence.
    let safe_end = floor_char_boundary(body, keep);
    s.push_str(&body[..safe_end]);
    s.push_str(TRUNCATE_MARKER);
    (s, true)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SqlDriver {
    Postgres,
    Mysql,
}

fn normalize_driver(driver: &str) -> Option<SqlDriver> {
    let lc = driver.trim().to_ascii_lowercase();
    match lc.as_str() {
        "postgres" | "postgresql" | "pg" => Some(SqlDriver::Postgres),
        "mysql" | "mariadb" => Some(SqlDriver::Mysql),
        _ => None,
    }
}

fn floor_char_boundary(s: &str, target: usize) -> usize {
    if target >= s.len() {
        return s.len();
    }
    let mut i = target;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_postgres_emits_analyze_buffers_format_json() {
        let r = prefix_explain_sql("postgres", "SELECT 1").unwrap();
        assert_eq!(r, "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) SELECT 1");
    }

    #[test]
    fn prefix_postgresql_alias_works() {
        let r = prefix_explain_sql("postgresql", "SELECT 1").unwrap();
        assert!(r.starts_with("EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)"));
    }

    #[test]
    fn prefix_pg_short_alias_works() {
        let r = prefix_explain_sql("pg", "SELECT 1").unwrap();
        assert!(r.starts_with("EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)"));
    }

    #[test]
    fn prefix_mysql_emits_format_json() {
        let r = prefix_explain_sql("mysql", "SELECT 1").unwrap();
        assert_eq!(r, "EXPLAIN FORMAT=JSON SELECT 1");
    }

    #[test]
    fn prefix_mariadb_alias_uses_mysql_form() {
        let r = prefix_explain_sql("mariadb", "SELECT 1").unwrap();
        assert_eq!(r, "EXPLAIN FORMAT=JSON SELECT 1");
    }

    #[test]
    fn driver_lookup_is_case_insensitive_and_trims() {
        assert!(prefix_explain_sql("  Postgres  ", "x").is_ok());
        assert!(prefix_explain_sql("MYSQL", "x").is_ok());
    }

    #[test]
    fn unknown_driver_errors_with_name_in_message() {
        let err = prefix_explain_sql("oracle", "SELECT 1").unwrap_err();
        assert_eq!(
            err,
            ExplainError::Unsupported {
                driver: "oracle".into()
            }
        );
        assert!(err.to_string().contains("oracle"));
    }

    #[test]
    fn sqlite_is_explicitly_unsupported_per_spec() {
        let err = prefix_explain_sql("sqlite", "SELECT 1").unwrap_err();
        match err {
            ExplainError::Unsupported { driver } => assert_eq!(driver, "sqlite"),
        }
    }

    #[test]
    fn bigquery_and_snowflake_unsupported() {
        assert!(prefix_explain_sql("bigquery", "x").is_err());
        assert!(prefix_explain_sql("snowflake", "x").is_err());
    }

    #[test]
    fn empty_sql_errors() {
        assert!(prefix_explain_sql("postgres", "").is_err());
        assert!(prefix_explain_sql("postgres", "   ").is_err());
        assert!(prefix_explain_sql("postgres", ";;;").is_err());
    }

    #[test]
    fn leading_semicolons_and_whitespace_stripped() {
        let r = prefix_explain_sql("postgres", "  ;\nSELECT 1").unwrap();
        assert_eq!(r, "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) SELECT 1");
    }

    #[test]
    fn cap_explain_body_passthrough_when_under_cap() {
        let body = "small body";
        let (out, truncated) = cap_explain_body(body);
        assert_eq!(out, body);
        assert!(!truncated);
    }

    #[test]
    fn cap_explain_body_at_cap_is_passthrough() {
        let body = "a".repeat(EXPLAIN_BODY_CAP);
        let (out, truncated) = cap_explain_body(&body);
        assert_eq!(out.len(), EXPLAIN_BODY_CAP);
        assert!(!truncated);
    }

    #[test]
    fn cap_explain_body_truncates_with_marker_when_over() {
        let body = "a".repeat(EXPLAIN_BODY_CAP + 1024);
        let (out, truncated) = cap_explain_body(&body);
        assert_eq!(out.len(), EXPLAIN_BODY_CAP);
        assert!(truncated);
        assert!(out.ends_with(TRUNCATE_MARKER));
    }

    #[test]
    fn cap_explain_body_handles_utf8_at_cap_boundary() {
        // Build a body that puts a multi-byte char straddling the
        // cap boundary; ensure we don't panic on a non-boundary
        // slice.
        let mut body = "a".repeat(EXPLAIN_BODY_CAP - 1);
        body.push('é'); // 2-byte char crossing the cap
        body.push_str(&"b".repeat(2048));
        let (out, truncated) = cap_explain_body(&body);
        assert!(truncated);
        assert!(out.is_char_boundary(out.len()));
        assert!(out.ends_with(TRUNCATE_MARKER));
    }

    #[test]
    fn explain_error_implements_std_error_trait() {
        let err = ExplainError::Unsupported { driver: "x".into() };
        let _: &dyn std::error::Error = &err;
    }
}
