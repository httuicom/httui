// coverage:exclude file — DB pool/exec/lookup or vault-store registry. Coverage requires live DB integration tests; owned by the critical-path test harness.

//! MySQL execute helpers used by the `DatabasePool::execute_*`
//! dispatchers.
//!
//! Extracted from `db::connections`. Owns MySQL SELECT
//! pagination, mutation, value binding,
//! and the type-aware row → JSON conversion (sqlx-mysql rejects
//! `i64` for UNSIGNED columns and rejects `String` for `JSON`,
//! so we dispatch by `column.type_info().name()`).

use base64::prelude::*;
use sqlx::{Column, Row, TypeInfo};

use super::connections::{ColumnInfo, JsonRow, QueryResult};
use super::pool::is_subqueryable_select;
use super::query_error::{sanitize_query_error_rich, QueryErrorInfo};

pub(super) async fn execute_select_mysql(
    pool: &sqlx::MySqlPool,
    sql: &str,
    bind_values: &[serde_json::Value],
    offset: u32,
    fetch_size: u32,
) -> Result<QueryResult, QueryErrorInfo> {
    let limit = (fetch_size + 1) as i64;
    let off = offset as i64;
    // SHOW / DESCRIBE / EXPLAIN aren't subqueryable in MySQL — run raw.
    let (paginated_sql, paginated) = if is_subqueryable_select(sql) {
        (
            format!("SELECT * FROM ({sql}) AS _p LIMIT {limit} OFFSET {off}"),
            true,
        )
    } else {
        (sql.to_string(), false)
    };

    let mut query = sqlx::query(&paginated_sql);
    for val in bind_values {
        query = bind_mysql_value(query, val);
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

    let json_rows: Vec<JsonRow> = rows.iter().map(mysql_row_to_json).collect();

    Ok(QueryResult {
        columns,
        rows: json_rows,
        has_more,
        rows_affected: None,
        is_select: true,
    })
}

pub(super) async fn execute_mutation_mysql(
    pool: &sqlx::MySqlPool,
    sql: &str,
    bind_values: &[serde_json::Value],
) -> Result<QueryResult, QueryErrorInfo> {
    let mut query = sqlx::query(sql);
    for val in bind_values {
        query = bind_mysql_value(query, val);
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

fn bind_mysql_value<'q>(
    query: sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments>,
    val: &'q serde_json::Value,
) -> sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments> {
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

fn mysql_row_to_json(row: &sqlx::mysql::MySqlRow) -> JsonRow {
    row.columns()
        .iter()
        .map(|col| mysql_value_to_json(row, col))
        .collect()
}

// Dispatch by column type name rather than a fallthrough chain of `try_get`.
// sqlx-mysql rejects `i64` for any UNSIGNED column and rejects `String` for
// `JSON` columns, so the old chain silently decoded BIGINT UNSIGNED as `bool`
// and JSON as raw wire bytes (9-byte length prefix + payload).
fn mysql_value_to_json(
    row: &sqlx::mysql::MySqlRow,
    col: &sqlx::mysql::MySqlColumn,
) -> serde_json::Value {
    use sqlx::ValueRef;
    let idx = col.ordinal();

    // Separate a real NULL from a decode failure. Without this distinction,
    // any type sqlx can't decode with the preferred Rust type would silently
    // collapse to JSON null.
    if let Ok(raw) = <sqlx::mysql::MySqlRow as sqlx::Row>::try_get_raw(row, idx) {
        if raw.is_null() {
            return serde_json::Value::Null;
        }
    }

    let ty = col.type_info().name();

    // Preferred per-type decoding; returns None if sqlx can't decode that type.
    // When it returns None, the fallback chain kicks in so the user still sees
    // something instead of null.
    decode_mysql_by_type(row, idx, ty).unwrap_or_else(|| mysql_fallback_decode(row, idx, ty))
}

fn decode_mysql_by_type(
    row: &sqlx::mysql::MySqlRow,
    idx: usize,
    ty: &str,
) -> Option<serde_json::Value> {
    Some(match ty {
        // sqlx-mysql reports TINYINT(1) as "BOOLEAN" (see ColumnType::name).
        // `i64::compatible` rejects it, so it has to go through `bool`.
        "BOOLEAN" => serde_json::Value::Bool(mysql_get::<bool>(row, idx)?),
        "TINYINT" | "SMALLINT" | "MEDIUMINT" | "INT" | "BIGINT" => {
            serde_json::Value::Number(mysql_get::<i64>(row, idx)?.into())
        }
        "TINYINT UNSIGNED" | "SMALLINT UNSIGNED" | "MEDIUMINT UNSIGNED" | "INT UNSIGNED" => {
            serde_json::Value::Number(mysql_get::<u32>(row, idx)?.into())
        }
        "BIGINT UNSIGNED" => serde_json::Value::Number(mysql_get::<u64>(row, idx)?.into()),
        "FLOAT" | "DOUBLE" => serde_json::json!(mysql_get::<f64>(row, idx)?),
        // DECIMAL: stringified to preserve precision without pulling in
        // `bigdecimal`/`rust_decimal` features.
        "DECIMAL" => serde_json::Value::String(mysql_get::<String>(row, idx)?),
        "VARCHAR" | "CHAR" | "TEXT" | "TINYTEXT" | "MEDIUMTEXT" | "LONGTEXT" | "ENUM" | "SET" => {
            serde_json::Value::String(mysql_get::<String>(row, idx)?)
        }
        // JSON: must go through `sqlx::types::Json` (requires `json` feature).
        // Decoding as `String` or `Vec<u8>` yields wire-format garbage.
        "JSON" => {
            let sqlx::types::Json(v) = row
                .try_get::<Option<sqlx::types::Json<serde_json::Value>>, _>(idx)
                .ok()
                .flatten()?;
            v
        }
        // Temporal types arrive as binary tuples over the MySQL binary protocol
        // (prepared statements), so `String` decoding fails. Go through chrono.
        "DATETIME" => serde_json::Value::String(
            mysql_get::<sqlx::types::chrono::NaiveDateTime>(row, idx)?
                .format("%Y-%m-%d %H:%M:%S%.f")
                .to_string(),
        ),
        "TIMESTAMP" => serde_json::Value::String(
            mysql_get::<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>(row, idx)?
                .to_rfc3339(),
        ),
        "DATE" => serde_json::Value::String(
            mysql_get::<sqlx::types::chrono::NaiveDate>(row, idx)?.to_string(),
        ),
        "TIME" => serde_json::Value::String(
            mysql_get::<sqlx::types::chrono::NaiveTime>(row, idx)?
                .format("%H:%M:%S%.f")
                .to_string(),
        ),
        "YEAR" => {
            if let Some(v) = mysql_get::<u16>(row, idx) {
                serde_json::Value::Number(v.into())
            } else {
                serde_json::Value::Number(mysql_get::<i64>(row, idx)?.into())
            }
        }
        "BLOB" | "TINYBLOB" | "MEDIUMBLOB" | "LONGBLOB" | "BINARY" | "VARBINARY" | "GEOMETRY"
        | "BIT" => {
            serde_json::Value::String(BASE64_STANDARD.encode(mysql_get::<Vec<u8>>(row, idx)?))
        }
        "NULL" => serde_json::Value::Null,
        _ => return None,
    })
}

// Fallback decoder invoked when the preferred decode for a known type fails
// or when a future/unmapped type name is encountered. At this point NULL has
// already been ruled out by the caller, so we try the two most permissive
// sqlx decoders and, as a last resort, emit a tagged marker so the user knows
// a value exists but couldn't be interpreted.
fn mysql_fallback_decode(row: &sqlx::mysql::MySqlRow, idx: usize, ty: &str) -> serde_json::Value {
    if let Some(s) = mysql_get::<String>(row, idx) {
        return serde_json::Value::String(s);
    }
    if let Some(bytes) = mysql_get::<Vec<u8>>(row, idx) {
        return match std::str::from_utf8(&bytes) {
            Ok(s) => serde_json::Value::String(s.to_string()),
            Err(_) => {
                serde_json::Value::String(format!("base64:{}", BASE64_STANDARD.encode(&bytes)))
            }
        };
    }
    serde_json::Value::String(format!("<unable to decode {}>", ty))
}

// Flattens `Result<Option<T>>` from sqlx into `Option<T>`. Safe to treat both
// NULL and decode errors as `None` here because the caller already handled
// real NULLs via `try_get_raw`.
fn mysql_get<T>(row: &sqlx::mysql::MySqlRow, idx: usize) -> Option<T>
where
    T: for<'r> sqlx::Decode<'r, sqlx::MySql> + sqlx::Type<sqlx::MySql>,
{
    row.try_get::<Option<T>, _>(idx).ok().flatten()
}
