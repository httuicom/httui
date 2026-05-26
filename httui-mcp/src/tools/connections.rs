use httui_core::db::connections::PoolManager;
use serde_json::json;
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;

pub async fn list_connections(pool: &SqlitePool) -> String {
    match httui_core::db::connections::list_connections(pool).await {
        Ok(conns) => {
            // Expose only non-sensitive metadata (no host, port, database_name, or username).
            let safe: Vec<serde_json::Value> = conns
                .iter()
                .map(|c| {
                    json!({
                        "id": c.id,
                        "name": c.name,
                        "driver": c.driver,
                    })
                })
                .collect();
            json!({"connections": safe}).to_string()
        }
        Err(e) => json!({"error": e}).to_string(),
    }
}

pub async fn get_db_schema(
    pool: &SqlitePool,
    conn_manager: &Arc<PoolManager>,
    connection_id: &str,
) -> String {
    // Try cached first
    match httui_core::db::schema_cache::get_cached_schema(pool, connection_id, 300).await {
        Ok(Some(entries)) => return json!({"schema": entries}).to_string(),
        Ok(None) => {} // cache miss — fall through to introspect
        Err(e) => return json!({"error": e}).to_string(),
    }

    match httui_core::db::schema_cache::introspect_schema(conn_manager, pool, connection_id).await {
        Ok(entries) => json!({"schema": entries}).to_string(),
        Err(e) => json!({"error": e}).to_string(),
    }
}

pub async fn test_connection(conn_manager: &Arc<PoolManager>, connection_id: &str) -> String {
    match conn_manager.test_connection(connection_id).await {
        Ok(()) => {
            json!({"success": format!("Connection {} is reachable", connection_id)}).to_string()
        }
        Err(e) => json!({"error": e}).to_string(),
    }
}
