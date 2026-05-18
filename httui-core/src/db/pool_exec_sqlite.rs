//! SQLite-specific execute helpers used by the `DatabasePool::execute_*`
//! dispatchers.
//!
//! Extracted from `db::connections`.
//! Owns SQLite SELECT pagination, mutation, value binding, and row →
//! JSON conversion.

use sqlx::{Column, Row, TypeInfo};

use super::connections::{ColumnInfo, JsonRow, QueryResult};
use super::pool::is_subqueryable_select;
use super::query_error::{sanitize_query_error_rich, QueryErrorInfo};

pub(super) async fn execute_select_sqlite(
    pool: &sqlx::SqlitePool,
    sql: &str,
    bind_values: &[serde_json::Value],
    offset: u32,
    fetch_size: u32,
) -> Result<QueryResult, QueryErrorInfo> {
    // Fetch one extra row to detect has_more
    let limit = (fetch_size + 1) as i64;
    let off = offset as i64;
    // EXPLAIN / PRAGMA / SHOW / DESCRIBE can't be subqueried, so run them
    // raw. We lose the `has_more` probe + pagination on those — fine,
    // those statements never need either.
    let (paginated_sql, paginated) = if is_subqueryable_select(sql) {
        (
            format!("SELECT * FROM ({sql}) LIMIT {limit} OFFSET {off}"),
            true,
        )
    } else {
        (sql.to_string(), false)
    };

    let mut query = sqlx::query(&paginated_sql);
    for val in bind_values {
        query = bind_sqlite_value(query, val);
    }

    let mut rows = query
        .fetch_all(pool)
        .await
        .map_err(|e| sanitize_query_error_rich(&e))?;

    // `has_more` only meaningful when we paginated with a +1 probe row.
    // Non-paginated runs (EXPLAIN/PRAGMA/SHOW/DESCRIBE) return their full
    // output and never have "more".
    let has_more = paginated && rows.len() > fetch_size as usize;
    if has_more {
        rows.pop(); // Remove the extra probe row
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

    Ok(QueryResult {
        columns,
        rows: json_rows,
        has_more,
        rows_affected: None,
        is_select: true,
    })
}

pub(super) async fn execute_mutation_sqlite(
    pool: &sqlx::SqlitePool,
    sql: &str,
    bind_values: &[serde_json::Value],
) -> Result<QueryResult, QueryErrorInfo> {
    let mut query = sqlx::query(sql);
    for val in bind_values {
        query = bind_sqlite_value(query, val);
    }

    let result = query
        .execute(pool)
        .await
        .map_err(|e| sanitize_query_error_rich(&e))?;

    Ok(QueryResult {
        columns: Vec::new(),
        rows: Vec::new(),
        has_more: false,
        rows_affected: Some(result.rows_affected()),
        is_select: false,
    })
}

fn bind_sqlite_value<'q>(
    query: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>>,
    val: &'q serde_json::Value,
) -> sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>> {
    // Non-primitive and out-of-range values already rejected by validate_bind_values
    match val {
        serde_json::Value::Null => query.bind(None::<String>),
        serde_json::Value::Bool(b) => query.bind(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                query.bind(i)
            } else {
                query.bind(
                    n.as_f64()
                        .expect("serde_json::Number is f64-representable when as_i64 fails"),
                )
            }
        }
        serde_json::Value::String(s) => query.bind(s.as_str()),
        _ => unreachable!("Non-primitive bind values rejected by validate_bind_values"),
    }
}

pub(crate) fn sqlite_row_to_json(row: &sqlx::sqlite::SqliteRow) -> JsonRow {
    row.columns()
        .iter()
        .map(|col| {
            let idx = col.ordinal();
            // Try types in order: integer, real, text, null
            if let Ok(v) = row.try_get::<i64, _>(idx) {
                serde_json::Value::Number(v.into())
            } else if let Ok(v) = row.try_get::<f64, _>(idx) {
                serde_json::json!(v)
            } else if let Ok(v) = row.try_get::<String, _>(idx) {
                serde_json::Value::String(v)
            } else if let Ok(v) = row.try_get::<bool, _>(idx) {
                serde_json::Value::Bool(v)
            } else {
                serde_json::Value::Null
            }
        })
        .collect()
}
