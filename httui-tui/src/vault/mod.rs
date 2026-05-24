//! Vault resolution.
//!
//! The active vault lives in the shared SQLite registry
//! (`httui_core::vaults`). `resolve` reads it and validates the path
//! still exists on disk; when nothing is registered (or the previously
//! active path is gone), it returns `None` so the caller can route the
//! user into the empty-state flow (see [`crate::empty_state`]).

use sqlx::sqlite::SqlitePool;
use std::path::PathBuf;
use tracing::warn;

use crate::error::TuiResult;

pub mod helpers;

#[derive(Debug)]
pub struct ResolvedVault {
    pub vault: PathBuf,
}

/// Read the active vault from the database. Returns `Ok(None)` when
/// no vault is registered, or when the previously-active path no
/// longer exists on disk — the caller routes that into the empty-state
/// bootstrap.
pub async fn resolve(pool: &SqlitePool) -> TuiResult<Option<ResolvedVault>> {
    let Some(active) = httui_core::vaults::get_active_vault(pool).await? else {
        return Ok(None);
    };
    let path = PathBuf::from(&active);
    if path.is_dir() {
        return Ok(Some(ResolvedVault { vault: path }));
    }
    warn!(?path, "active vault no longer exists on disk");
    Ok(None)
}

/// `~/foo` → `/Users/joao/foo`. Only the leading `~/` is expanded;
/// `~user` (other-user shorthand) is intentionally not supported.
pub fn expand_tilde(s: &str) -> String {
    if let Some(rest) = s.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{home}/{rest}");
        }
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use httui_core::db::init_db;
    use httui_core::vaults::set_active_vault;
    use tempfile::TempDir;

    #[tokio::test]
    async fn resolves_active_from_db() {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        let pool = init_db(data.path()).await.unwrap();

        set_active_vault(&pool, &vault.path().to_string_lossy())
            .await
            .unwrap();

        let resolved = resolve(&pool).await.unwrap().expect("vault registered");
        assert_eq!(resolved.vault, vault.path().to_path_buf());
    }

    #[tokio::test]
    async fn returns_none_when_registry_is_empty() {
        let data = TempDir::new().unwrap();
        let pool = init_db(data.path()).await.unwrap();

        assert!(resolve(&pool).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn returns_none_when_active_vault_path_is_gone() {
        let data = TempDir::new().unwrap();
        let pool = init_db(data.path()).await.unwrap();

        // Register a path then remove it from disk.
        let ghost = TempDir::new().unwrap();
        let ghost_path = ghost.path().to_string_lossy().to_string();
        set_active_vault(&pool, &ghost_path).await.unwrap();
        drop(ghost);

        assert!(resolve(&pool).await.unwrap().is_none());
    }

    #[test]
    fn expand_tilde_replaces_only_leading() {
        std::env::set_var("HOME", "/Users/test");
        assert_eq!(expand_tilde("~/foo"), "/Users/test/foo");
        assert_eq!(expand_tilde("/abs/~/path"), "/abs/~/path");
        assert_eq!(expand_tilde("relative/path"), "relative/path");
    }
}
