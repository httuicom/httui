//! Per-vault registry for `ConnectionsStore` and `EnvironmentsStore`.
//! Caches one store pair per vault path; keyed by `app_config.active_vault`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use httui_core::config::get_config;
use httui_core::db::connections::Connection;
use httui_core::db::lookup::ConnectionLookup;
use httui_core::vault_config::{
    user_store::default_user_config_path, ConnectionsStore, EnvironmentsStore,
};
use sqlx::sqlite::SqlitePool;

/// Pair of stores for the same vault root.
#[derive(Clone)]
pub struct VaultStores {
    pub connections: Arc<ConnectionsStore>,
    pub environments: Arc<EnvironmentsStore>,
}

/// Registry of per-vault store pairs. Single instance lives in Tauri state.
pub struct VaultStoreRegistry {
    cache: RwLock<HashMap<PathBuf, VaultStores>>,
}

impl VaultStoreRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            cache: RwLock::new(HashMap::new()),
        })
    }

    /// Resolve stores for the currently-active vault (`app_config.active_vault`).
    /// Returns an error if no active vault is set.
    pub async fn for_active_vault(&self, pool: &SqlitePool) -> Result<VaultStores, String> {
        let vault_path = get_config(pool, "active_vault")
            .await
            .map_err(|e| format!("read active_vault: {e}"))?
            .ok_or_else(|| "No active vault — open a vault first".to_string())?;
        let vault_root = PathBuf::from(vault_path);
        self.for_vault(vault_root).await
    }

    /// Get-or-create cached stores for a specific vault path.
    pub async fn for_vault(&self, vault_root: PathBuf) -> Result<VaultStores, String> {
        {
            let cache = self.cache.read().await;
            if let Some(stores) = cache.get(&vault_root) {
                return Ok(stores.clone());
            }
        }

        let user_config = default_user_config_path()?;
        let stores = VaultStores {
            connections: ConnectionsStore::new(vault_root.clone()),
            environments: EnvironmentsStore::new(vault_root.clone(), user_config),
        };

        {
            let mut cache = self.cache.write().await;
            cache.entry(vault_root).or_insert_with(|| stores.clone());
        }

        Ok(stores)
    }

    /// Drop cached stores for a vault. Safe to call even if not cached.
    pub async fn invalidate_vault(&self, vault_root: &std::path::Path) {
        let mut cache = self.cache.write().await;
        cache.remove(vault_root);
    }
}

/// `ConnectionLookup` adapter that resolves the active vault via `VaultStoreRegistry`.
pub struct VaultRegistryLookup {
    pool: SqlitePool,
    registry: Arc<VaultStoreRegistry>,
}

impl VaultRegistryLookup {
    pub fn new(pool: SqlitePool, registry: Arc<VaultStoreRegistry>) -> Arc<Self> {
        Arc::new(Self { pool, registry })
    }
}

#[async_trait]
impl ConnectionLookup for VaultRegistryLookup {
    async fn lookup(&self, name: &str) -> Result<Option<Connection>, String> {
        let stores = self.registry.for_active_vault(&self.pool).await?;
        stores.connections.lookup(name).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn for_vault_caches_same_instance() {
        let registry = VaultStoreRegistry::new();
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();

        // First call instantiates.
        let s1 = registry.for_vault(path.clone()).await.unwrap();
        // Second call returns the cached instance.
        let s2 = registry.for_vault(path.clone()).await.unwrap();

        assert!(Arc::ptr_eq(&s1.connections, &s2.connections));
        assert!(Arc::ptr_eq(&s1.environments, &s2.environments));
    }

    #[tokio::test]
    async fn invalidate_drops_cached_stores() {
        let registry = VaultStoreRegistry::new();
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();

        let s1 = registry.for_vault(path.clone()).await.unwrap();
        registry.invalidate_vault(&path).await;
        let s2 = registry.for_vault(path.clone()).await.unwrap();

        // After invalidation we get a fresh instance, not the cached one.
        assert!(!Arc::ptr_eq(&s1.connections, &s2.connections));
    }

    #[tokio::test]
    async fn different_vaults_get_different_stores() {
        let registry = VaultStoreRegistry::new();
        let a = TempDir::new().unwrap();
        let b = TempDir::new().unwrap();

        let sa = registry.for_vault(a.path().to_path_buf()).await.unwrap();
        let sb = registry.for_vault(b.path().to_path_buf()).await.unwrap();

        assert!(!Arc::ptr_eq(&sa.connections, &sb.connections));
        assert!(!Arc::ptr_eq(&sa.environments, &sb.environments));
    }

    async fn pool_with_active_vault(active: Option<&str>) -> (TempDir, SqlitePool) {
        let dir = TempDir::new().unwrap();
        let pool = httui_core::db::init_db(&dir.path().join("notes.db"))
            .await
            .unwrap();
        if let Some(p) = active {
            httui_core::config::set_config(&pool, "active_vault", p)
                .await
                .unwrap();
        }
        (dir, pool)
    }

    #[tokio::test]
    async fn for_active_vault_resolves_against_app_config() {
        let v = TempDir::new().unwrap();
        let vault_str = v.path().to_string_lossy().into_owned();
        let (_db_dir, pool) = pool_with_active_vault(Some(&vault_str)).await;
        let registry = VaultStoreRegistry::new();

        let stores = registry.for_active_vault(&pool).await.unwrap();
        let again = registry.for_vault(v.path().to_path_buf()).await.unwrap();
        // for_active_vault populated the cache — the second call hits it.
        assert!(Arc::ptr_eq(&stores.connections, &again.connections));
    }

    #[tokio::test]
    async fn for_active_vault_errors_when_unset() {
        let (_db_dir, pool) = pool_with_active_vault(None).await;
        let registry = VaultStoreRegistry::new();
        let err = registry
            .for_active_vault(&pool)
            .await
            .err()
            .expect("expected an error");
        assert!(err.contains("No active vault"), "got: {err}");
    }

    #[tokio::test]
    async fn vault_registry_lookup_returns_none_for_unknown_connection() {
        let v = TempDir::new().unwrap();
        let vault_str = v.path().to_string_lossy().into_owned();
        let (_db_dir, pool) = pool_with_active_vault(Some(&vault_str)).await;
        let registry = VaultStoreRegistry::new();
        let lookup = VaultRegistryLookup::new(pool, registry);
        let res = lookup.lookup("missing").await.unwrap();
        assert!(res.is_none());
    }

    #[tokio::test]
    async fn vault_registry_lookup_propagates_no_active_vault_error() {
        let (_db_dir, pool) = pool_with_active_vault(None).await;
        let registry = VaultStoreRegistry::new();
        let lookup = VaultRegistryLookup::new(pool, registry);
        let err = lookup
            .lookup("anything")
            .await
            .expect_err("expected an error");
        assert!(err.contains("No active vault"), "got: {err}");
    }
}
