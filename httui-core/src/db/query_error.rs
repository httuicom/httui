//! Query error sanitization + driver-specific location extraction.
//!
//! Extracted from `db::connections`. Owns:
//!
//! - `QueryErrorInfo` + `QueryErrorLocation` data types
//! - `sanitize_query_error` / `sanitize_query_error_rich` — strip
//!   credentials from sqlx errors before surfacing to the UI
//! - Driver-specific location helpers
//!   (`extract_error_location` for Postgres `PgErrorPosition` /
//!   MySQL "at line N" messages)
//! - `enrich_error_with_query` — resolves Postgres byte position to
//!   (line, col) once the executor knows the original SQL text

/// Sanitize query errors — expose database error messages (safe) but
/// strip connection details. Used by callers that only need the
/// string form; the rich variant carries `QueryErrorLocation`.
pub(crate) fn sanitize_query_error(e: sqlx::Error) -> String {
    sanitize_query_error_rich(&e).message
}

/// Cursor location inside the source SQL where the error was reported.
/// Populated from driver-specific metadata (Postgres `position`, MySQL
/// `near … at line N`); `None` when the driver didn't expose it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct QueryErrorLocation {
    pub line: Option<u32>,
    pub column: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct QueryErrorInfo {
    pub message: String,
    pub location: QueryErrorLocation,
}

impl std::fmt::Display for QueryErrorInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl From<QueryErrorInfo> for String {
    fn from(value: QueryErrorInfo) -> String {
        value.message
    }
}

impl QueryErrorInfo {
    /// Convenience proxy so existing `.contains(...)` assertions in tests
    /// keep reading naturally without being forced onto `.message.contains(...)`.
    pub fn contains(&self, needle: &str) -> bool {
        self.message.contains(needle)
    }
}

/// Sanitize a sqlx error AND pull out line/column when the driver exposes
/// them. The string message is the same as `sanitize_query_error` so the
/// existing "Query failed: …" prefix is preserved.
pub fn sanitize_query_error_rich(e: &sqlx::Error) -> QueryErrorInfo {
    match e {
        sqlx::Error::Database(db_err) => {
            let msg = format!("Query failed: {}", db_err.message());
            let location = extract_error_location(db_err.as_ref());
            QueryErrorInfo {
                message: msg,
                location,
            }
        }
        _ => QueryErrorInfo {
            message: "Query failed".to_string(),
            location: QueryErrorLocation::default(),
        },
    }
}

/// Turn a Postgres 1-indexed char position into (line, column). Returns
/// `(1, 1)` if the position overruns the query (shouldn't happen in
/// practice).
fn position_to_line_col(query: &str, position: u32) -> (u32, u32) {
    if position == 0 {
        return (1, 1);
    }
    let target = (position as usize).saturating_sub(1);
    let mut line = 1u32;
    let mut col = 1u32;
    for (idx, ch) in query.char_indices() {
        if idx >= target {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn extract_error_location(db_err: &dyn sqlx::error::DatabaseError) -> QueryErrorLocation {
    // Postgres: downcast to access the PgErrorPosition helper. The position
    // is an offset into the original query string (1-indexed). Callers
    // know the query and convert to line/col via `enrich_error_with_query`.
    if let Some(p) = db_err
        .try_downcast_ref::<sqlx::postgres::PgDatabaseError>()
        .and_then(|pg| pg.position())
        .and_then(|pos| match pos {
            sqlx::postgres::PgErrorPosition::Original(p) => Some(p),
            _ => None,
        })
    {
        // Stash position as raw column for now; caller converts
        // once it has the query text.
        return QueryErrorLocation {
            line: None,
            column: Some(p as u32),
        };
    }
    // MySQL: message often contains "near '…' at line N".
    let msg = db_err.message();
    if let Some(line) = mysql_line_from_message(msg) {
        return QueryErrorLocation {
            line: Some(line),
            column: None,
        };
    }
    QueryErrorLocation::default()
}

fn mysql_line_from_message(msg: &str) -> Option<u32> {
    // Example: "You have an error in your SQL syntax; ... at line 3".
    let lower = msg.to_ascii_lowercase();
    let idx = lower.find(" at line ")?;
    let tail = &msg[idx + " at line ".len()..];
    let digits: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// After capture, resolve Postgres-style raw byte position into (line, col)
/// using the query text the executor sent. MySQL line (already extracted
/// from the message) passes through unchanged.
pub fn enrich_error_with_query(info: &mut QueryErrorInfo, query: &str) {
    if info.location.line.is_none() {
        if let Some(pos) = info.location.column {
            let (l, c) = position_to_line_col(query, pos);
            info.location.line = Some(l);
            info.location.column = Some(c);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_to_line_col_zero_returns_origin() {
        assert_eq!(position_to_line_col("SELECT 1", 0), (1, 1));
    }

    #[test]
    fn position_to_line_col_walks_lines() {
        let q = "SELECT 1\nFROM t\nWHERE x = 2";
        // position 1 = 'S' (col 1, line 1)
        assert_eq!(position_to_line_col(q, 1), (1, 1));
        // position 10 = 'F' on line 2 (col 1)
        assert_eq!(position_to_line_col(q, 10), (2, 1));
        // position 17 = 'W' on line 3 (col 1)
        assert_eq!(position_to_line_col(q, 17), (3, 1));
    }

    #[test]
    fn mysql_line_from_message_extracts() {
        assert_eq!(
            mysql_line_from_message("You have an error in your SQL syntax; ... at line 3"),
            Some(3)
        );
        assert_eq!(mysql_line_from_message("no line marker"), None);
    }

    #[test]
    fn sanitize_query_error_rich_handles_non_db_errors() {
        let e = sqlx::Error::PoolTimedOut;
        let info = sanitize_query_error_rich(&e);
        assert_eq!(info.message, "Query failed");
        assert!(info.location.line.is_none());
    }

    #[test]
    fn enrich_error_with_query_resolves_pg_position() {
        let mut info = QueryErrorInfo {
            message: "x".to_string(),
            location: QueryErrorLocation {
                line: None,
                column: Some(10),
            },
        };
        enrich_error_with_query(&mut info, "SELECT 1\nFROM t");
        assert_eq!(info.location.line, Some(2));
    }
}
