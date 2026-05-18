// coverage:exclude file — DB pool/exec/lookup or vault-store registry. Coverage requires live DB integration tests; owned by the critical-path test harness.

//! Postgres execute helpers used by the `DatabasePool::execute_*`
//! dispatchers.
//!
//! Extracted from `db::connections`.
//! Owns Postgres SELECT pagination, mutation, value binding, and row →
//! JSON conversion. `?` → `$N` normalization happens here so callers
//! can write driver-agnostic SQL with `?` and the dispatcher picks
//! the right rewrite per driver.

use sqlx::{Column, Row, TypeInfo};

use super::connections::{ColumnInfo, JsonRow, QueryResult};
use super::pool::is_subqueryable_select;
use super::query_error::{sanitize_query_error_rich, QueryErrorInfo};
use super::sql_scanner::normalize_placeholders_to_pg;

pub(super) async fn execute_select_pg(
    pool: &sqlx::PgPool,
    sql: &str,
    bind_values: &[serde_json::Value],
    offset: u32,
    fetch_size: u32,
) -> Result<QueryResult, QueryErrorInfo> {
    let pg_sql = normalize_placeholders_to_pg(sql);

    let limit = (fetch_size + 1) as i64;
    let off = offset as i64;
    // EXPLAIN / SHOW can't be a relation source in Postgres, so skip the
    // pagination wrapper for them.
    let (paginated_sql, paginated) = if is_subqueryable_select(&pg_sql) {
        (
            format!("SELECT * FROM ({pg_sql}) AS _p LIMIT {limit} OFFSET {off}"),
            true,
        )
    } else {
        (pg_sql, false)
    };

    let mut query = sqlx::query(&paginated_sql);
    for val in bind_values {
        query = bind_pg_value(query, val);
    }

    let mut rows = query
        .fetch_all(pool)
        .await
        .map_err(|e| sanitize_query_error_rich(&e))?;

    let has_more = paginated && rows.len() > fetch_size as usize;
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

    let json_rows: Vec<JsonRow> = rows.iter().map(pg_row_to_json).collect();

    Ok(QueryResult {
        columns,
        rows: json_rows,
        has_more,
        rows_affected: None,
        is_select: true,
    })
}

pub(super) async fn execute_mutation_pg(
    pool: &sqlx::PgPool,
    sql: &str,
    bind_values: &[serde_json::Value],
) -> Result<QueryResult, QueryErrorInfo> {
    let pg_sql = normalize_placeholders_to_pg(sql);
    let mut query = sqlx::query(&pg_sql);
    for val in bind_values {
        query = bind_pg_value(query, val);
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

fn bind_pg_value<'q>(
    query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    val: &'q serde_json::Value,
) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
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

fn pg_row_to_json(row: &sqlx::postgres::PgRow) -> JsonRow {
    row.columns()
        .iter()
        .map(|col| {
            let idx = col.ordinal();
            if let Ok(v) = row.try_get::<i64, _>(idx) {
                serde_json::Value::Number(v.into())
            } else if let Ok(v) = row.try_get::<i32, _>(idx) {
                serde_json::Value::Number(v.into())
            } else if let Ok(v) = row.try_get::<f64, _>(idx) {
                serde_json::json!(v)
            } else if let Ok(v) = row.try_get::<bool, _>(idx) {
                serde_json::Value::Bool(v)
            } else if let Ok(v) = row.try_get::<String, _>(idx) {
                serde_json::Value::String(v)
            } else {
                serde_json::Value::Null
            }
        })
        .collect()
}
