//! Vault registry: list of known workspaces + active selection.
//!
//! A *vault* is a filesystem directory containing markdown notes. The
//! app remembers a list of vaults the user has opened and which one is
//! currently active, persisted in the `app_config` SQLite table:
//!
//! - Key `vaults` → JSON array of absolute paths.
//! - Key `active_vault` → single absolute path (must be in `vaults`).
//!
//! Mirror of the desktop logic in `src/lib/tauri/commands.ts:79-108`.
//! Living in the core lets desktop, TUI and MCP converge on the same
//! semantics.

use sqlx::sqlite::SqlitePool;

use crate::config::{get_config, set_config};
use crate::error::{CoreError, CoreResult};

const VAULTS_KEY: &str = "vaults";
const ACTIVE_VAULT_KEY: &str = "active_vault";

/// Return all registered vault paths in insertion order. Empty list on
/// first run.
pub async fn list_vaults(pool: &SqlitePool) -> CoreResult<Vec<String>> {
    let raw = get_config(pool, VAULTS_KEY).await?;
    match raw {
        None => Ok(Vec::new()),
        Some(s) if s.is_empty() => Ok(Vec::new()),
        Some(s) => serde_json::from_str(&s).map_err(CoreError::from),
    }
}

/// Add a vault to the registry if not already present. Idempotent.
pub async fn add_vault(pool: &SqlitePool, path: &str) -> CoreResult<()> {
    let mut vaults = list_vaults(pool).await?;
    if !vaults.iter().any(|v| v == path) {
        vaults.push(path.to_string());
        let raw = serde_json::to_string(&vaults)?;
        set_config(pool, VAULTS_KEY, &raw).await?;
    }
    Ok(())
}

/// Remove a vault from the registry. If it was active, clear the
/// active selection so callers can re-prompt.
pub async fn remove_vault(pool: &SqlitePool, path: &str) -> CoreResult<()> {
    let vaults: Vec<String> = list_vaults(pool)
        .await?
        .into_iter()
        .filter(|v| v != path)
        .collect();
    let raw = serde_json::to_string(&vaults)?;
    set_config(pool, VAULTS_KEY, &raw).await?;

    if get_active_vault(pool).await? == Some(path.to_string()) {
        set_config(pool, ACTIVE_VAULT_KEY, "").await?;
    }
    Ok(())
}

/// Return the currently active vault, if any.
pub async fn get_active_vault(pool: &SqlitePool) -> CoreResult<Option<String>> {
    let raw = get_config(pool, ACTIVE_VAULT_KEY).await?;
    Ok(raw.filter(|s| !s.is_empty()))
}

/// Select an active vault. Adds it to the registry first (so callers
/// don't have to remember to). After this, `get_active_vault` returns
/// `Some(path)`.
pub async fn set_active_vault(pool: &SqlitePool, path: &str) -> CoreResult<()> {
    add_vault(pool, path).await?;
    set_config(pool, ACTIVE_VAULT_KEY, path).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use tempfile::TempDir;

    async fn setup() -> (SqlitePool, TempDir) {
        let tmp = TempDir::new().unwrap();
        let pool = init_db(tmp.path()).await.unwrap();
        (pool, tmp)
    }

    #[tokio::test]
    async fn empty_registry_returns_empty_list() {
        let (pool, _tmp) = setup().await;
        assert!(list_vaults(&pool).await.unwrap().is_empty());
        assert!(get_active_vault(&pool).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn add_vault_is_idempotent() {
        let (pool, _tmp) = setup().await;
        add_vault(&pool, "/a").await.unwrap();
        add_vault(&pool, "/a").await.unwrap();
        add_vault(&pool, "/b").await.unwrap();
        let vs = list_vaults(&pool).await.unwrap();
        assert_eq!(vs, vec!["/a".to_string(), "/b".to_string()]);
    }

    #[tokio::test]
    async fn set_active_registers_and_marks() {
        let (pool, _tmp) = setup().await;
        set_active_vault(&pool, "/notas").await.unwrap();
        assert_eq!(
            get_active_vault(&pool).await.unwrap(),
            Some("/notas".to_string())
        );
        assert_eq!(
            list_vaults(&pool).await.unwrap(),
            vec!["/notas".to_string()]
        );
    }

    #[tokio::test]
    async fn set_active_switches_between_existing() {
        let (pool, _tmp) = setup().await;
        set_active_vault(&pool, "/a").await.unwrap();
        set_active_vault(&pool, "/b").await.unwrap();
        assert_eq!(
            get_active_vault(&pool).await.unwrap(),
            Some("/b".to_string())
        );
        // Both stay in the list.
        assert_eq!(
            list_vaults(&pool).await.unwrap(),
            vec!["/a".to_string(), "/b".to_string()]
        );
    }

    #[tokio::test]
    async fn remove_vault_drops_from_list() {
        let (pool, _tmp) = setup().await;
        add_vault(&pool, "/a").await.unwrap();
        add_vault(&pool, "/b").await.unwrap();
        remove_vault(&pool, "/a").await.unwrap();
        assert_eq!(list_vaults(&pool).await.unwrap(), vec!["/b".to_string()]);
    }

    #[tokio::test]
    async fn remove_active_vault_clears_active() {
        let (pool, _tmp) = setup().await;
        set_active_vault(&pool, "/a").await.unwrap();
        remove_vault(&pool, "/a").await.unwrap();
        assert!(get_active_vault(&pool).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn remove_non_active_keeps_active() {
        let (pool, _tmp) = setup().await;
        add_vault(&pool, "/a").await.unwrap();
        set_active_vault(&pool, "/b").await.unwrap();
        remove_vault(&pool, "/a").await.unwrap();
        assert_eq!(
            get_active_vault(&pool).await.unwrap(),
            Some("/b".to_string())
        );
    }
}
