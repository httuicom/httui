// coverage:exclude file — Tauri command shells take `tauri::State<'_, T>`
// and aren't unit-testable in isolation. The pure helpers
// (DTO conversion) are tested below; substantive logic
// (file-backed CRUD, atomic write, secret resolution) lives in
// `httui_core::vault_config::connections_store` at >80% coverage.
// Same rationale as `commands/environments.rs` and
// `vault_config_commands.rs`. Retires when the per-domain
// command harness lands.

//! Connection Tauri commands — file-backed cutover.
//!
//! Wire-compat with the legacy `db::connections::ConnectionPublic`
//! shape so the React frontend doesn't need changes:
//!
//! - `Connection.id == name` (file-backed natural key promoted)
//! - `created_at` / `updated_at` returned as empty strings (file
//!   metadata could surface them via FS mtime; deferred — UI doesn't
//!   render either field today)
//! - `last_tested_at` returned as `None` for now; the legacy
//!   `last_tested_at` write was dropped in Phase 3 (PoolManager no
//!   longer touches the legacy SQLite `connections` table). A
//!   per-machine status cache is a v1.x follow-up if a UI need
//!   emerges.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use tauri::State;

use std::path::PathBuf;

use httui_core::connection_uses::{find_connection_uses, ConnectionUse};
use httui_core::db::connections::PoolManager;
use httui_core::vault_config::connection_views::ConnectionPublic as FileConnectionPublic;
use httui_core::vault_config::connections_store::{CreateConnectionInput, UpdateConnectionInput};

use super::vault_stores::VaultStoreRegistry;

/// Wire-compat: matches the legacy `db::connections::ConnectionPublic`
/// shape. `id` carries the connection name post-cutover.
#[derive(Debug, Clone, Serialize)]
pub struct ConnectionPublic {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub host: Option<String>,
    pub port: Option<i64>,
    pub database_name: Option<String>,
    pub username: Option<String>,
    pub has_password: bool,
    pub ssl_mode: Option<String>,
    pub timeout_ms: i64,
    pub query_timeout_ms: i64,
    pub ttl_seconds: i64,
    pub max_pool_size: i64,
    pub is_readonly: bool,
    pub last_tested_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Frontend payload for `create_connection`. Mirrors the legacy
/// `db::connections::CreateConnection` fields so the React form
/// doesn't change.
#[derive(Debug, Deserialize)]
pub struct CreateConnection {
    pub name: String,
    pub driver: String,
    pub host: Option<String>,
    pub port: Option<i64>,
    pub database_name: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub ssl_mode: Option<String>,
    pub timeout_ms: Option<i64>,
    pub query_timeout_ms: Option<i64>,
    pub ttl_seconds: Option<i64>,
    pub max_pool_size: Option<i64>,
    pub is_readonly: Option<bool>,
}

/// Frontend payload for `update_connection`. All fields optional —
/// only the provided ones are written.
#[derive(Debug, Deserialize)]
pub struct UpdateConnection {
    pub name: Option<String>,
    pub driver: Option<String>,
    pub host: Option<String>,
    pub port: Option<i64>,
    pub database_name: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub ssl_mode: Option<String>,
    pub timeout_ms: Option<i64>,
    pub query_timeout_ms: Option<i64>,
    pub ttl_seconds: Option<i64>,
    pub max_pool_size: Option<i64>,
    pub is_readonly: Option<bool>,
}

// Defaults mirror `vault_config::connections_store::to_legacy` —
// the file-backed format doesn't yet carry these advanced fields,
// and the canvas connection form doesn't expose them
// either. When per-connection overrides become user-facing, the
// file format grows a `[<conn>.advanced]` section.
const DEFAULT_TIMEOUT_MS: i64 = 10_000;
const DEFAULT_QUERY_TIMEOUT_MS: i64 = 30_000;
const DEFAULT_TTL_SECONDS: i64 = 300;
const DEFAULT_MAX_POOL_SIZE: i64 = 5;

fn to_wire(c: FileConnectionPublic) -> ConnectionPublic {
    ConnectionPublic {
        id: c.name.clone(),
        name: c.name,
        driver: c.driver,
        host: c.host,
        port: c.port.map(i64::from),
        database_name: c.database_name,
        username: c.username,
        has_password: c.has_password,
        ssl_mode: c.ssl_mode,
        timeout_ms: DEFAULT_TIMEOUT_MS,
        query_timeout_ms: DEFAULT_QUERY_TIMEOUT_MS,
        ttl_seconds: DEFAULT_TTL_SECONDS,
        max_pool_size: DEFAULT_MAX_POOL_SIZE,
        is_readonly: c.is_readonly,
        last_tested_at: None,
        created_at: String::new(),
        updated_at: String::new(),
    }
}

#[tauri::command]
pub async fn list_connections(
    pool: State<'_, SqlitePool>,
    registry: State<'_, Arc<VaultStoreRegistry>>,
) -> Result<Vec<ConnectionPublic>, String> {
    let stores = registry.for_active_vault(&pool).await?;
    let conns = stores.connections.list_public().await?;
    Ok(conns.into_iter().map(to_wire).collect())
}

/// `i64` (frontend wire type for port) → `u16` (file format type).
/// Returns an error if the port is out of range. The frontend never
/// emits negative or out-of-range values, so this is a safety net
/// rather than a user-facing error path.
fn to_port(v: Option<i64>) -> Result<Option<u16>, String> {
    match v {
        None => Ok(None),
        Some(p) if (0..=u16::MAX as i64).contains(&p) => Ok(Some(p as u16)),
        Some(p) => Err(format!("port {p} out of range 0..=65535")),
    }
}

#[tauri::command]
pub async fn create_connection(
    pool: State<'_, SqlitePool>,
    registry: State<'_, Arc<VaultStoreRegistry>>,
    input: CreateConnection,
) -> Result<ConnectionPublic, String> {
    let stores = registry.for_active_vault(&pool).await?;
    let created = stores
        .connections
        .create(CreateConnectionInput {
            name: input.name,
            driver: input.driver,
            host: input.host,
            port: to_port(input.port)?,
            database_name: input.database_name,
            username: input.username,
            password: input.password,
            ssl_mode: input.ssl_mode,
            is_readonly: input.is_readonly,
            description: None,
        })
        .await?;
    // Advanced fields (`timeout_ms`, `query_timeout_ms`, `ttl_seconds`,
    // `max_pool_size`) on the wire are accepted but currently ignored —
    // the file format doesn't carry them. Defaults from
    // `vault_config::connections_store::to_legacy` apply at pool-build
    // time. A future `[<conn>.advanced]` TOML section adds per-conn
    // overrides; tracked in `tech-debt.md` as a v1.x follow-up.
    Ok(to_wire(created))
}

#[tauri::command]
pub async fn update_connection(
    pool: State<'_, SqlitePool>,
    registry: State<'_, Arc<VaultStoreRegistry>>,
    conn_manager: State<'_, Arc<PoolManager>>,
    id: String,
    input: UpdateConnection,
) -> Result<ConnectionPublic, String> {
    // Rename is not supported in the file-backed store (the connection
    // name is the natural key). If frontend passes a new name, fail
    // explicitly so the form can route through delete+create instead.
    if let Some(new_name) = &input.name {
        if new_name != &id {
            return Err(format!(
                "rename not supported in v1 ({} → {}); recreate instead",
                id, new_name
            ));
        }
    }
    let stores = registry.for_active_vault(&pool).await?;
    let updated = stores
        .connections
        .update(
            &id,
            UpdateConnectionInput {
                driver: input.driver,
                host: input.host,
                port: to_port(input.port)?,
                database_name: input.database_name,
                username: input.username,
                password: input.password,
                ssl_mode: input.ssl_mode,
                is_readonly: input.is_readonly,
                description: None,
            },
        )
        .await?;
    conn_manager.invalidate(&id).await;
    Ok(to_wire(updated))
}

#[tauri::command]
pub async fn delete_connection(
    pool: State<'_, SqlitePool>,
    registry: State<'_, Arc<VaultStoreRegistry>>,
    conn_manager: State<'_, Arc<PoolManager>>,
    id: String,
) -> Result<(), String> {
    let stores = registry.for_active_vault(&pool).await?;
    conn_manager.invalidate(&id).await;
    stores.connections.delete(&id).await
}

#[tauri::command]
pub async fn test_connection(
    conn_manager: State<'_, Arc<PoolManager>>,
    id: String,
) -> Result<(), String> {
    conn_manager.test_connection(&id).await
}

/// Vault-wide grep for db-block fences referencing `connection=<name>`.
/// Powers the "Used in runbooks" panel in ConnectionsPage. Wraps
/// `httui_core::connection_uses::find_connection_uses`
/// substantive logic + tests live there.
#[tauri::command]
pub async fn find_connection_uses_cmd(
    vault_path: String,
    connection_name: String,
) -> Result<Vec<ConnectionUse>, String> {
    let root = PathBuf::from(vault_path);
    Ok(find_connection_uses(&root, &connection_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_file_public(name: &str) -> FileConnectionPublic {
        FileConnectionPublic {
            name: name.to_string(),
            driver: "postgres".to_string(),
            host: Some("localhost".to_string()),
            port: Some(5432),
            database_name: Some("test".to_string()),
            username: Some("user".to_string()),
            has_password: true,
            ssl_mode: None,
            is_readonly: false,
            description: None,
        }
    }

    #[test]
    fn to_wire_promotes_name_to_id() {
        let wire = to_wire(sample_file_public("payments-db"));
        assert_eq!(wire.id, "payments-db");
        assert_eq!(wire.name, "payments-db");
    }

    #[test]
    fn to_wire_zeroes_legacy_timestamps() {
        let wire = to_wire(sample_file_public("x"));
        assert!(wire.created_at.is_empty());
        assert!(wire.updated_at.is_empty());
        assert!(wire.last_tested_at.is_none());
    }

    #[test]
    fn to_wire_preserves_connection_metadata() {
        let wire = to_wire(sample_file_public("y"));
        assert_eq!(wire.driver, "postgres");
        assert_eq!(wire.host.as_deref(), Some("localhost"));
        assert_eq!(wire.port, Some(5432));
        assert!(wire.has_password);
    }

    #[test]
    fn to_wire_emits_default_advanced_fields() {
        // The file format doesn't carry advanced timeout/pool fields
        // yet (form doesn't expose them either). Defaults
        // mirror `vault_config::connections_store::to_legacy`.
        let wire = to_wire(sample_file_public("z"));
        assert_eq!(wire.timeout_ms, DEFAULT_TIMEOUT_MS);
        assert_eq!(wire.query_timeout_ms, DEFAULT_QUERY_TIMEOUT_MS);
        assert_eq!(wire.ttl_seconds, DEFAULT_TTL_SECONDS);
        assert_eq!(wire.max_pool_size, DEFAULT_MAX_POOL_SIZE);
    }

    #[test]
    fn to_port_handles_full_range() {
        assert_eq!(to_port(None).unwrap(), None);
        assert_eq!(to_port(Some(0)).unwrap(), Some(0));
        assert_eq!(to_port(Some(5432)).unwrap(), Some(5432));
        assert_eq!(to_port(Some(65535)).unwrap(), Some(65535));
        assert!(to_port(Some(65536)).is_err());
        assert!(to_port(Some(-1)).is_err());
    }
}
