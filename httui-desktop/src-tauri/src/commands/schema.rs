// coverage:exclude file — Tauri command shells with no testable logic without a Tauri runtime.

//! Schema introspection Tauri commands. `introspect_schema` has a 5-second freshness guard
//! so the UI can call it idempotently without hammering the target database.

use std::sync::Arc;

use sqlx::sqlite::SqlitePool;
use tauri::State;

use httui_core::db::connections::PoolManager;
use httui_core::db::schema_cache::{self, SchemaEntry};

#[tauri::command]
pub async fn introspect_schema(
    pool: State<'_, SqlitePool>,
    conn_manager: State<'_, Arc<PoolManager>>,
    connection_id: String,
) -> Result<Vec<SchemaEntry>, String> {
    if let Ok(Some(cached)) = schema_cache::get_cached_schema(&pool, &connection_id, 5).await {
        return Ok(cached);
    }
    schema_cache::introspect_schema(&conn_manager, &pool, &connection_id).await
}

/// Read-only access to the cached schema for `connection_id`. Returns
/// `None` if no cache hit younger than `ttl_seconds` (default 300s).
#[tauri::command]
pub async fn get_cached_schema(
    pool: State<'_, SqlitePool>,
    connection_id: String,
    ttl_seconds: Option<i64>,
) -> Result<Option<Vec<SchemaEntry>>, String> {
    schema_cache::get_cached_schema(&pool, &connection_id, ttl_seconds.unwrap_or(300)).await
}
