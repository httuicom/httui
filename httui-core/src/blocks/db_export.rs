//! Pure serializers for a db-block `Select` result. Used by the export
//! menu in both the TUI and (eventually) the desktop — the desktop's
//! `src/lib/blocks/db-export.ts` is the reference implementation; this
//! module mirrors its semantics line-for-line so a vault behaves
//! identically across runtimes.
//!
//! The functions take `(columns, rows)` slices instead of a full
//! [`DbResult`] enum so callers that already destructured `Select` (or
//! that build columns/rows from cached JSON) don't need to reconstruct
//! the variant. `serde_json::Value` is the row shape because that's
//! what the executor actually emits — columns map to keys.
//!
//! Mutation / error results have no tabular form to export and the
//! caller is expected to gate the menu on `!result.is_select()` or
//! equivalent. We don't emit a friendly placeholder on purpose: the
//! caller's job is to never feed us non-tabular input.

use crate::db::connections::ColumnInfo;
use serde_json::Value;

/// RFC 4180-ish CSV. Quotes any field containing CR, LF, comma, or
/// double-quote; doubles internal quotes. `null` values become empty
/// fields (NOT the literal string "null") so spreadsheets see a real
/// blank cell. Complex values (objects/arrays) round-trip as compact
/// JSON inside a quoted field — same as the desktop emitter.
pub fn to_csv(columns: &[ColumnInfo], rows: &[Value]) -> String {
    let mut out = String::new();
    out.push_str(
        &columns
            .iter()
            .map(|c| csv_escape(&c.name))
            .collect::<Vec<_>>()
            .join(","),
    );
    out.push('\n');
    for row in rows {
        let line = columns
            .iter()
            .map(|c| csv_escape(&format_cell_csv(row.get(&c.name))))
            .collect::<Vec<_>>()
            .join(",");
        out.push_str(&line);
        out.push('\n');
    }
    out
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\r') || value.contains('\n') {
        let escaped = value.replace('"', "\"\"");
        format!("\"{escaped}\"")
    } else {
        value.to_string()
    }
}

fn format_cell_csv(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(other) => other.to_string(),
    }
}

/// Pretty-printed JSON array of row objects. Trailing newline included
/// so writing the result to a file leaves a POSIX-friendly final byte.
pub fn to_json(rows: &[Value]) -> String {
    let mut s = serde_json::to_string_pretty(rows).unwrap_or_else(|_| "[]".to_string());
    s.push('\n');
    s
}

/// GitHub-flavored Markdown table. Pipes and backslashes inside cells
/// are escaped so the table stays syntactically valid; embedded
/// newlines become spaces so each cell stays on one row.
pub fn to_markdown(columns: &[ColumnInfo], rows: &[Value]) -> String {
    let mut out = String::new();
    let header = columns
        .iter()
        .map(|c| md_escape(&c.name))
        .collect::<Vec<_>>()
        .join(" | ");
    out.push_str(&format!("| {header} |\n"));

    let separator = columns
        .iter()
        .map(|_| "---".to_string())
        .collect::<Vec<_>>()
        .join(" | ");
    out.push_str(&format!("| {separator} |\n"));

    for row in rows {
        let body = columns
            .iter()
            .map(|c| md_escape(&format_cell_md(row.get(&c.name))))
            .collect::<Vec<_>>()
            .join(" | ");
        out.push_str(&format!("| {body} |\n"));
    }
    out
}

fn md_escape(value: &str) -> String {
    let mut s = value.replace('\\', "\\\\").replace('|', "\\|");
    // Collapse CRLF/CR/LF to a single space so cells stay one-line.
    s = s.replace("\r\n", " ").replace(['\r', '\n'], " ");
    s
}

fn format_cell_md(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(other) => other.to_string(),
    }
}

/// Emit one `INSERT INTO <table> (cols) VALUES (...)` per row. Strings
/// get SQL-quoted with single quotes (escaping internal `'` by
/// doubling). Objects/arrays are serialized as JSON and stored inside
/// a quoted string literal — works on Postgres `jsonb`, MySQL `JSON`,
/// and SQLite (no native JSON, but the literal still parses).
///
/// `table_name` falls back to the literal `<table>` when the caller
/// can't infer one (e.g. a CTE-only query). Users can find/replace
/// after pasting.
pub fn to_inserts(columns: &[ColumnInfo], rows: &[Value], table_name: &str) -> String {
    let safe_table = if table_name.is_empty() {
        "<table>".to_string()
    } else {
        table_name.to_string()
    };
    let cols = columns
        .iter()
        .map(|c| ident_or_quote(&c.name))
        .collect::<Vec<_>>()
        .join(", ");

    let mut out = String::new();
    for row in rows {
        let values = columns
            .iter()
            .map(|c| sql_literal(row.get(&c.name)))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "INSERT INTO {safe_table} ({cols}) VALUES ({values});\n"
        ));
    }
    out
}

fn ident_or_quote(name: &str) -> String {
    let valid = !name.is_empty()
        && name
            .chars()
            .next()
            .map(|c| c.is_ascii_alphabetic() || c == '_')
            .unwrap_or(false)
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if valid {
        name.to_string()
    } else {
        let escaped = name.replace('"', "\"\"");
        format!("\"{escaped}\"")
    }
}

fn sql_literal(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => "NULL".to_string(),
        Some(Value::Number(n)) => {
            // Reject NaN/Infinity (JSON technically can't carry them
            // but defensive — JS export emits NULL in that case too).
            if let Some(f) = n.as_f64() {
                if !f.is_finite() {
                    return "NULL".to_string();
                }
            }
            n.to_string()
        }
        Some(Value::Bool(true)) => "TRUE".to_string(),
        Some(Value::Bool(false)) => "FALSE".to_string(),
        Some(Value::String(s)) => format!("'{}'", s.replace('\'', "''")),
        Some(other) => {
            let json = other.to_string();
            format!("'{}'", json.replace('\'', "''"))
        }
    }
}

/// Best-effort pull of a table name from a SQL query: first identifier
/// after the first `FROM` keyword (case-insensitive). Used as the
/// default for INSERT export. Returns `None` when no `FROM` clause is
/// present (e.g. a `VALUES` query, a CTE-only construct) — callers
/// fall back to `<table>` and let the user fix it up.
pub fn infer_table_name(sql: &str) -> Option<String> {
    // Strip line comments and block comments first so a `FROM` that
    // only appears inside a comment doesn't get picked up.
    let cleaned = strip_sql_comments(sql);
    let lower = cleaned.to_ascii_lowercase();
    let mut search_from = 0usize;
    while let Some(pos) = lower[search_from..].find("from") {
        let abs = search_from + pos;
        let before = if abs == 0 {
            None
        } else {
            cleaned[..abs].chars().last()
        };
        let after = cleaned[abs + 4..].chars().next();
        let is_word_boundary_before = before.map(|c| !is_ident_char(c)).unwrap_or(true);
        let is_word_boundary_after = after.map(|c| !is_ident_char(c)).unwrap_or(true);
        if is_word_boundary_before && is_word_boundary_after {
            // Skip whitespace, then read an identifier (optionally
            // dotted: schema.table).
            let rest: &str = &cleaned[abs + 4..];
            let rest = rest.trim_start();
            let mut end = 0usize;
            for (i, ch) in rest.char_indices() {
                if is_ident_char(ch) || ch == '.' {
                    end = i + ch.len_utf8();
                } else {
                    break;
                }
            }
            if end > 0 {
                return Some(rest[..end].to_string());
            }
        }
        search_from = abs + 4;
    }
    None
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Strip `-- line comments` and `/* block comments */`. Not a full SQL
/// lexer (doesn't track string literals), but good enough for the
/// `FROM` heuristic: a `FROM` inside a string is rare and a false
/// positive on `infer_table_name` only changes the default INSERT
/// table — never executes anything.
fn strip_sql_comments(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    let bytes = sql.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Line comment.
        if i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        // Block comment.
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// `true` when there's at least one column AND one row to serialize.
/// Callers gate the export menu on this so an empty result set
/// doesn't produce a header-only CSV / empty markdown table.
pub fn has_exportable_rows(columns: &[ColumnInfo], rows: &[Value]) -> bool {
    !columns.is_empty() && !rows.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn col(name: &str) -> ColumnInfo {
        ColumnInfo {
            name: name.into(),
            type_name: "text".into(),
        }
    }

    fn cols(names: &[&str]) -> Vec<ColumnInfo> {
        names.iter().map(|n| col(n)).collect()
    }

    #[test]
    fn csv_emits_header_then_rows() {
        let c = cols(&["id", "name"]);
        let rows = vec![
            json!({"id": 1, "name": "alice"}),
            json!({"id": 2, "name": "bob"}),
        ];
        let csv = to_csv(&c, &rows);
        assert_eq!(csv, "id,name\n1,alice\n2,bob\n");
    }

    #[test]
    fn csv_quotes_fields_with_comma_quote_or_newline() {
        let c = cols(&["v"]);
        let rows = vec![
            json!({"v": "a,b"}),             // comma → quoted
            json!({"v": "she said \"hi\""}), // quote → quoted + doubled
            json!({"v": "line1\nline2"}),    // newline → quoted
        ];
        let csv = to_csv(&c, &rows);
        assert!(csv.contains("\"a,b\""));
        assert!(csv.contains("\"she said \"\"hi\"\"\""));
        assert!(csv.contains("\"line1\nline2\""));
    }

    #[test]
    fn csv_null_becomes_empty_field_not_string_null() {
        let c = cols(&["v"]);
        let rows = vec![json!({"v": null}), json!({})]; // missing key also empty
        let csv = to_csv(&c, &rows);
        assert_eq!(csv, "v\n\n\n");
    }

    #[test]
    fn csv_complex_value_round_trips_as_json() {
        let c = cols(&["meta"]);
        let rows = vec![json!({"meta": {"a": 1}})];
        let csv = to_csv(&c, &rows);
        // Compact JSON serialization — no whitespace. The embedded
        // double quotes force CSV quoting (RFC 4180): the field is
        // wrapped and internal quotes are doubled.
        assert!(csv.contains("\"{\"\"a\"\":1}\""), "got: {csv}");
    }

    #[test]
    fn json_pretty_array_with_trailing_newline() {
        let rows = vec![json!({"id": 1})];
        let s = to_json(&rows);
        assert!(s.starts_with("[\n"));
        assert!(s.ends_with("]\n"));
        assert!(s.contains("\"id\": 1"));
    }

    #[test]
    fn markdown_emits_header_separator_and_rows() {
        let c = cols(&["id", "name"]);
        let rows = vec![json!({"id": 1, "name": "alice"})];
        let md = to_markdown(&c, &rows);
        assert_eq!(md, "| id | name |\n| --- | --- |\n| 1 | alice |\n");
    }

    #[test]
    fn markdown_escapes_pipes_and_backslashes_in_cells() {
        let c = cols(&["v"]);
        let rows = vec![json!({"v": "a|b\\c"})];
        let md = to_markdown(&c, &rows);
        assert!(md.contains("a\\|b\\\\c"));
    }

    #[test]
    fn markdown_collapses_newlines_to_spaces() {
        let c = cols(&["v"]);
        let rows = vec![json!({"v": "line1\nline2"})];
        let md = to_markdown(&c, &rows);
        // The CRLF/LF collapse keeps each row on a single visual line
        // — important because a literal newline inside a markdown cell
        // breaks the table.
        assert!(md.contains("line1 line2"));
        assert!(!md.contains("line1\nline2"));
    }

    #[test]
    fn inserts_basic_row() {
        let c = cols(&["id", "name"]);
        let rows = vec![json!({"id": 1, "name": "alice"})];
        let sql = to_inserts(&c, &rows, "users");
        assert_eq!(sql, "INSERT INTO users (id, name) VALUES (1, 'alice');\n");
    }

    #[test]
    fn inserts_quote_strings_doubling_internal_quote() {
        let c = cols(&["s"]);
        let rows = vec![json!({"s": "it's"})];
        let sql = to_inserts(&c, &rows, "t");
        assert!(sql.contains("'it''s'"));
    }

    #[test]
    fn inserts_null_becomes_literal_null() {
        let c = cols(&["v"]);
        let rows = vec![json!({"v": null}), json!({})];
        let sql = to_inserts(&c, &rows, "t");
        // Both literal null and missing key render NULL (parity with
        // the JS implementation — distinguishing the two would need
        // schema-aware defaults, which we don't have here).
        assert!(sql.contains("VALUES (NULL);"));
        let n_count = sql.matches("VALUES (NULL);").count();
        assert_eq!(n_count, 2);
    }

    #[test]
    fn inserts_bool_uses_true_false() {
        let c = cols(&["b"]);
        let rows = vec![json!({"b": true}), json!({"b": false})];
        let sql = to_inserts(&c, &rows, "t");
        assert!(sql.contains("VALUES (TRUE);"));
        assert!(sql.contains("VALUES (FALSE);"));
    }

    #[test]
    fn inserts_complex_value_serialized_as_json_string() {
        let c = cols(&["meta"]);
        let rows = vec![json!({"meta": {"a": 1}})];
        let sql = to_inserts(&c, &rows, "t");
        // The JSON sits inside a single-quoted SQL literal so the
        // postgres jsonb/mysql JSON column cast picks it up cleanly.
        assert!(sql.contains("'{\"a\":1}'"));
    }

    #[test]
    fn inserts_falls_back_to_table_placeholder_when_empty_name() {
        let c = cols(&["id"]);
        let rows = vec![json!({"id": 1})];
        let sql = to_inserts(&c, &rows, "");
        assert!(sql.contains("INSERT INTO <table>"));
    }

    #[test]
    fn inserts_quote_unsafe_column_names() {
        // A column named `from` would conflict with the SQL keyword
        // (and a name starting with a digit isn't a valid identifier).
        // We don't try to detect keywords — just non-ident characters
        // and bad first chars — but the important property is that
        // the output is always re-parseable, i.e. the column list
        // round-trips.
        let c = cols(&["weird name"]);
        let rows = vec![json!({"weird name": 1})];
        let sql = to_inserts(&c, &rows, "t");
        assert!(sql.contains("(\"weird name\")"));
    }

    #[test]
    fn infer_simple_select() {
        assert_eq!(
            infer_table_name("SELECT * FROM users WHERE id = 1"),
            Some("users".into())
        );
    }

    #[test]
    fn infer_dotted_schema_table() {
        assert_eq!(
            infer_table_name("SELECT * FROM public.users"),
            Some("public.users".into())
        );
    }

    #[test]
    fn infer_case_insensitive() {
        assert_eq!(
            infer_table_name("select * from users"),
            Some("users".into())
        );
        assert_eq!(
            infer_table_name("SELECT * From Users"),
            Some("Users".into())
        );
    }

    #[test]
    fn infer_ignores_from_inside_line_comment() {
        // `-- FROM hidden` is a comment — the real table is
        // `users`. Without comment stripping we'd return "hidden".
        let sql = "-- FROM hidden\nSELECT * FROM users";
        assert_eq!(infer_table_name(sql), Some("users".into()));
    }

    #[test]
    fn infer_ignores_from_inside_block_comment() {
        let sql = "/* FROM hidden */ SELECT * FROM users";
        assert_eq!(infer_table_name(sql), Some("users".into()));
    }

    #[test]
    fn infer_returns_none_when_no_from_clause() {
        assert_eq!(infer_table_name("SELECT 1"), None);
        assert_eq!(infer_table_name("VALUES (1, 2)"), None);
    }

    #[test]
    fn infer_skips_substring_match_inside_identifier() {
        // `SELECT fromage FROM foods` — the `fromage` should not
        // be picked as the table; we want `foods`. Word boundary
        // check guards against this.
        let sql = "SELECT fromage FROM foods";
        assert_eq!(infer_table_name(sql), Some("foods".into()));
    }

    #[test]
    fn empty_result_not_exportable() {
        assert!(!has_exportable_rows(&[], &[]));
        assert!(!has_exportable_rows(&cols(&["id"]), &[]));
        assert!(!has_exportable_rows(&[], &[json!({"id": 1})]));
    }

    #[test]
    fn populated_result_is_exportable() {
        assert!(has_exportable_rows(&cols(&["id"]), &[json!({"id": 1})]));
    }
}
