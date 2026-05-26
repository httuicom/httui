// coverage:exclude file — DB pool/exec/lookup or vault-store registry. Coverage requires live DB integration tests; owned by the critical-path test harness.

//! Postgres execute helpers used by the `DatabasePool::execute_*`
//! dispatchers.
//!
//! Extracted from `db::connections`.
//! Owns Postgres SELECT pagination, mutation, value binding, and row →
//! JSON conversion. `?` → `$N` normalization happens here so callers
//! can write driver-agnostic SQL with `?` and the dispatcher picks
//! the right rewrite per driver.

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use sqlx::{Column, Row, TypeInfo, ValueRef};

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
        .map(|col| pg_decode_value(row, col))
        .collect()
}

/// Decode one Postgres column into `serde_json::Value`. Dispatches by
/// `type_info().name()` (e.g. `INT8`, `NUMERIC`, `TIMESTAMPTZ`, `JSONB`)
/// so each Postgres type maps to the right sqlx decoder. The older
/// `try_get` cascade lost NUMERIC/TIMESTAMPTZ/JSONB/UUID/BYTEA — every
/// non-matching branch fell through to `Null`, which the UI rendered
/// as `(null)` even when the column held a real value.
fn pg_decode_value(
    row: &sqlx::postgres::PgRow,
    col: &sqlx::postgres::PgColumn,
) -> serde_json::Value {
    let idx = col.ordinal();
    // Explicit NULL check up front so a real NULL doesn't get masked by
    // a decode failure (e.g. NUMERIC of a NULL value would error).
    if let Ok(raw) = row.try_get_raw(idx) {
        if raw.is_null() {
            return serde_json::Value::Null;
        }
    }
    let ty = col.type_info().name().to_string();
    pg_decode_by_type(row, idx, &ty).unwrap_or_else(|| pg_fallback_decode(row, idx, &ty))
}

fn pg_decode_by_type(
    row: &sqlx::postgres::PgRow,
    idx: usize,
    ty: &str,
) -> Option<serde_json::Value> {
    use sqlx::types::{chrono, BigDecimal, Json, Uuid};
    Some(match ty {
        "BOOL" => serde_json::Value::Bool(row.try_get::<bool, _>(idx).ok()?),
        "INT2" => serde_json::Value::Number(row.try_get::<i16, _>(idx).ok()?.into()),
        "INT4" => serde_json::Value::Number(row.try_get::<i32, _>(idx).ok()?.into()),
        "INT8" => serde_json::Value::Number(row.try_get::<i64, _>(idx).ok()?.into()),
        "FLOAT4" => serde_json::json!(row.try_get::<f32, _>(idx).ok()? as f64),
        "FLOAT8" => serde_json::json!(row.try_get::<f64, _>(idx).ok()?),
        // NUMERIC: decode via BigDecimal (arbitrary precision) then
        // emit JSON number when an f64 round-trip is exact; fall back
        // to string for values that would lose precision. Common money
        // amounts (`499.80`, `0.1`) export as proper numbers.
        "NUMERIC" => {
            super::pool::decimal_to_json(&row.try_get::<BigDecimal, _>(idx).ok()?.to_string())
        }
        "TEXT" | "VARCHAR" | "BPCHAR" | "NAME" | "CHAR" | "CITEXT" => {
            serde_json::Value::String(row.try_get::<String, _>(idx).ok()?)
        }
        "JSON" | "JSONB" => {
            let Json(v) = row.try_get::<Json<serde_json::Value>, _>(idx).ok()?;
            v
        }
        "UUID" => serde_json::Value::String(row.try_get::<Uuid, _>(idx).ok()?.to_string()),
        "DATE" => {
            serde_json::Value::String(row.try_get::<chrono::NaiveDate, _>(idx).ok()?.to_string())
        }
        "TIME" => serde_json::Value::String(
            row.try_get::<chrono::NaiveTime, _>(idx)
                .ok()?
                .format("%H:%M:%S%.f")
                .to_string(),
        ),
        "TIMESTAMP" => serde_json::Value::String(
            row.try_get::<chrono::NaiveDateTime, _>(idx)
                .ok()?
                .format("%Y-%m-%d %H:%M:%S%.f")
                .to_string(),
        ),
        "TIMESTAMPTZ" => serde_json::Value::String(
            row.try_get::<chrono::DateTime<chrono::Utc>, _>(idx)
                .ok()?
                .to_rfc3339(),
        ),
        "BYTEA" => {
            serde_json::Value::String(BASE64_STANDARD.encode(row.try_get::<Vec<u8>, _>(idx).ok()?))
        }
        _ => return None,
    })
}

/// Fallback when the type-specific decoder doesn't fire (unknown type,
/// or a known type whose decoder errored for whatever reason). At this
/// point NULL is already ruled out by the caller; try the permissive
/// String decoder, then BYTEA-as-base64, then surface a tagged marker
/// so the user sees `something` instead of silent `(null)`.
fn pg_fallback_decode(row: &sqlx::postgres::PgRow, idx: usize, ty: &str) -> serde_json::Value {
    if let Ok(s) = row.try_get::<String, _>(idx) {
        return serde_json::Value::String(s);
    }
    if let Ok(bytes) = row.try_get::<Vec<u8>, _>(idx) {
        return match std::str::from_utf8(&bytes) {
            Ok(s) => serde_json::Value::String(s.to_string()),
            Err(_) => {
                serde_json::Value::String(format!("base64:{}", BASE64_STANDARD.encode(&bytes)))
            }
        };
    }
    serde_json::Value::String(format!("<unable to decode {}>", ty))
}
