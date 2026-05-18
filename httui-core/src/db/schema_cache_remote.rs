// coverage:exclude file — Remote DB introspection (Postgres + MySQL).
// Each `introspect_*` is a thin SQL→struct loop driven by sqlx; the
// pure SchemaEntry mapping (`build_pg_entry`, `build_mysql_entry`,
// `first_string_or_bytes_lossy`) and the SQL constants stay in
// `schema_cache.rs` and are unit-tested there. Exercising these
// async helpers needs a live Postgres / MySQL pool — owned by the
// critical-path / DB integration test harness.

use sqlx::Row;

use super::schema_cache::{
    build_mysql_entry, build_pg_entry, first_string_or_bytes_lossy, SchemaEntry,
    MYSQL_INTROSPECT_SQL, POSTGRES_INTROSPECT_SQL,
};

pub async fn introspect_postgres(pool: &sqlx::PgPool) -> Result<Vec<SchemaEntry>, String> {
    let rows = sqlx::query(POSTGRES_INTROSPECT_SQL)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(rows
        .iter()
        .map(|row| {
            build_pg_entry(
                row.try_get::<String, _>("table_schema").ok(),
                row.get("table_name"),
                row.get("column_name"),
                row.try_get("data_type").ok(),
            )
        })
        .collect())
}

fn mysql_str(row: &sqlx::mysql::MySqlRow, col: &str) -> Option<String> {
    first_string_or_bytes_lossy(
        row.try_get::<String, _>(col),
        row.try_get::<Vec<u8>, _>(col),
    )
}

pub async fn introspect_mysql(pool: &sqlx::MySqlPool) -> Result<Vec<SchemaEntry>, String> {
    let rows = sqlx::query(MYSQL_INTROSPECT_SQL)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(rows
        .iter()
        .filter_map(|row| {
            build_mysql_entry(
                mysql_str(row, "TABLE_SCHEMA"),
                mysql_str(row, "TABLE_NAME"),
                mysql_str(row, "COLUMN_NAME"),
                mysql_str(row, "DATA_TYPE"),
            )
        })
        .collect())
}
