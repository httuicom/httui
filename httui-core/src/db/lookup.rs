//! Connection lookup abstraction. Decouples `PoolManager` from the
//! concrete storage backend: production reads from
//! `vault_config::ConnectionsStore` (file-backed), tests use a mock,
//! and the legacy `SqlitePool`-backed lookup remains available as
//! an adapter for the SQLite test fixtures that have not yet
//! migrated to file-backed (Epic 20a Story 01 cleanup).
//!
//! Closes the DIP item from `tech-debt.md` —
//! `PoolManager::new(app_pool: SqlitePool, ...)` no longer hardwires
//! SQLite as the source of connection records (Epic 19 Story 02
//! Phase 3; audit-015).

use std::sync::Arc;

use async_trait::async_trait;
use sqlx::sqlite::SqlitePool;

use super::connections::Connection;
use crate::vault_config::ConnectionsStore;

/// Resolve a `Connection` by name (primary key in the file-backed
/// world) or by id (primary key in the legacy SQLite world — adapters
/// accept both forms during the cutover window).
#[async_trait]
pub trait ConnectionLookup: Send + Sync {
    async fn lookup(&self, key: &str) -> Result<Option<Connection>, String>;
}

/// File-backed lookup — the production path. Delegates to
/// `ConnectionsStore::get_legacy`, which already returns the exact
/// `Connection` shape `PoolManager` expects.
#[async_trait]
impl ConnectionLookup for ConnectionsStore {
    async fn lookup(&self, name: &str) -> Result<Option<Connection>, String> {
        self.get_legacy(name).await
    }
}

/// Adapter for tests/fixtures that still seed `connections` rows
/// into the legacy SQLite-backed table. Production code does NOT
/// use this — desktop / tui / mcp construct a `ConnectionsStore`
/// instead.
pub struct SqliteLookup {
    pool: SqlitePool,
}

impl SqliteLookup {
    pub fn new(pool: SqlitePool) -> Arc<Self> {
        Arc::new(Self { pool })
    }
}

#[async_trait]
impl ConnectionLookup for SqliteLookup {
    async fn lookup(&self, key: &str) -> Result<Option<Connection>, String> {
        // Legacy lookup accepted ids; allow either form to bridge the
        // gap during the cutover.
        super::connections::get_connection(&self.pool, key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn empty_pool() -> SqlitePool {
        // Use the real init_db migrations so the `connections` schema
        // exactly matches production. A tempdir keeps each test
        // isolated.
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("notes.db");
        let pool = super::super::init_db(&path).await.unwrap();
        // Leak the tempdir into the pool's lifetime — it lives until
        // the pool is dropped at end of test, which keeps the
        // db file around.
        std::mem::forget(dir);
        pool
    }

    #[tokio::test]
    async fn sqlite_lookup_returns_none_for_unknown_key() {
        let pool = empty_pool().await;
        let lookup = SqliteLookup::new(pool);
        let res = lookup.lookup("does-not-exist").await.unwrap();
        assert!(res.is_none());
    }

    #[tokio::test]
    async fn sqlite_lookup_constructs_via_arc_helper() {
        let pool = empty_pool().await;
        let arc = SqliteLookup::new(pool);
        // `new` returns an Arc — strong count starts at 1.
        assert_eq!(Arc::strong_count(&arc), 1);
    }

    #[tokio::test]
    async fn connections_store_lookup_returns_none_when_file_missing() {
        // ConnectionsStore is the production lookup path. With an empty
        // vault root (no `connections.toml`), `get_legacy` resolves to
        // `Ok(None)`, so the trait impl mirrors the same shape.
        let dir = TempDir::new().unwrap();
        let store = ConnectionsStore::new(dir.path());
        let res = ConnectionLookup::lookup(store.as_ref(), "missing")
            .await
            .unwrap();
        assert!(res.is_none());
    }
}
