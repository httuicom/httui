//! Pool lifecycle, TTL eviction, status emission.
//!
//! Extracted from `db::connections` (Epic 20a Story 01 — first split).
//! `connections.rs` was 2894 L mixing 7 concerns; this file owns the
//! pool-management one. Holds an `Arc<dyn ConnectionLookup>` (file-
//! backed in production via `vault_config::ConnectionsStore`,
//! `SqliteLookup` in legacy tests) — no direct SQLite coupling for
//! connection records.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use sqlx::sqlite::SqlitePool;
use tokio::sync::RwLock;

use super::lookup::ConnectionLookup;
use super::pool::{create_pool, DatabasePool};

/// Trait for emitting connection status events.
/// The Tauri app provides an AppHandle-based implementation; the MCP
/// binary (and tests) use None.
pub trait StatusEmitter: Send + Sync {
    fn emit_connection_status(&self, connection_id: &str, name: &str, status: &str);
}

/// Session-scoped host:port override for a connection (V11 cenário 2).
///
/// In-memory on the frontend (`connectionSessionOverride` store), passed
/// per DB execution through `DbParams`. Never persisted — the vault
/// connection record is untouched. An overridden run gets its own pool
/// under a composite cache key so the base pool stays clean and clearing
/// the override transparently falls back to it on the next run.
#[derive(Debug, Clone, Default)]
pub struct HostPortOverride {
    pub host: Option<String>,
    pub port: Option<i64>,
}

impl HostPortOverride {
    /// True when neither field is set — treated as "no override" so
    /// callers fall back to the plain base-connection pool path.
    pub fn is_empty(&self) -> bool {
        self.host.is_none() && self.port.is_none()
    }

    /// Stable suffix appended to the connection id to key the
    /// override-specific pool. Distinct host:port pairs get distinct
    /// pools; a cleared override reuses the base `connection_id` key.
    fn cache_suffix(&self) -> String {
        format!(
            "#ovr:{}:{}",
            self.host.as_deref().unwrap_or("-"),
            self.port
                .map(|p| p.to_string())
                .unwrap_or_else(|| "-".into()),
        )
    }
}

pub struct PoolManager {
    /// Resolves `Connection` records by name. File-backed in production
    /// (`ConnectionsStore`); SQLite adapter in legacy tests
    /// (`SqliteLookup`). See `db/lookup.rs`.
    lookup: Arc<dyn ConnectionLookup>,
    /// Retained only for `cleanup_query_log` (the `query_log` SQLite
    /// table; future Epic 20a Story owns the move-out and the field
    /// disappears with it).
    app_pool: SqlitePool,
    pools: RwLock<HashMap<String, PoolEntry>>,
    emitter: Option<Arc<dyn StatusEmitter>>,
}

struct PoolEntry {
    pool: Arc<DatabasePool>,
    name: String,
    last_used: Instant,
    ttl_seconds: u64,
    query_timeout_ms: u64,
}

impl PoolManager {
    pub fn new_with_emitter(
        lookup: Arc<dyn ConnectionLookup>,
        app_pool: SqlitePool,
        emitter: Arc<dyn StatusEmitter>,
    ) -> Self {
        Self {
            lookup,
            app_pool,
            pools: RwLock::new(HashMap::new()),
            emitter: Some(emitter),
        }
    }

    /// Create without event emitter (for MCP server and tests).
    pub fn new_standalone(lookup: Arc<dyn ConnectionLookup>, app_pool: SqlitePool) -> Self {
        Self {
            lookup,
            app_pool,
            pools: RwLock::new(HashMap::new()),
            emitter: None,
        }
    }

    pub fn app_pool(&self) -> &SqlitePool {
        &self.app_pool
    }

    pub async fn get_pool(&self, connection_id: &str) -> Result<Arc<DatabasePool>, String> {
        self.get_pool_keyed(connection_id, connection_id, None)
            .await
    }

    /// Like `get_pool`, but when a non-empty `HostPortOverride` is
    /// supplied the pool is created against the overridden host/port and
    /// cached under a composite key — the base `connection_id` pool is
    /// never mutated, so non-overridden runs (and a later cleared
    /// override) keep using it untouched. (V11 cenário 2.)
    pub async fn get_pool_with_override(
        &self,
        connection_id: &str,
        ov: Option<&HostPortOverride>,
    ) -> Result<Arc<DatabasePool>, String> {
        match ov {
            Some(o) if !o.is_empty() => {
                let key = format!("{connection_id}{}", o.cache_suffix());
                self.get_pool_keyed(&key, connection_id, Some(o)).await
            }
            _ => self.get_pool(connection_id).await,
        }
    }

    /// Single get-or-create path shared by `get_pool` (cache_key ==
    /// connection_id, no override) and `get_pool_with_override`
    /// (composite cache_key + host/port mutation before pool creation).
    async fn get_pool_keyed(
        &self,
        cache_key: &str,
        connection_id: &str,
        ov: Option<&HostPortOverride>,
    ) -> Result<Arc<DatabasePool>, String> {
        // Check cache — write lock to update last_used on hit
        {
            let mut pools = self.pools.write().await;
            if let Some(entry) = pools.get_mut(cache_key) {
                entry.last_used = Instant::now();
                return Ok(entry.pool.clone());
            }
        }

        // Not cached — resolve connection and create pool
        let mut conn = self
            .lookup
            .lookup(connection_id)
            .await?
            .ok_or_else(|| format!("Connection '{}' not found", connection_id))?;

        if let Some(o) = ov {
            if let Some(h) = &o.host {
                conn.host = Some(h.clone());
            }
            if let Some(p) = o.port {
                conn.port = Some(p);
            }
        }

        let conn_name = conn.name.clone();
        let pool = Arc::new(create_pool(&conn).await?);

        {
            let mut pools = self.pools.write().await;
            pools.insert(
                cache_key.to_string(),
                PoolEntry {
                    pool: pool.clone(),
                    name: conn_name.clone(),
                    last_used: Instant::now(),
                    ttl_seconds: conn.ttl_seconds as u64,
                    query_timeout_ms: conn.query_timeout_ms as u64,
                },
            );
        }

        if let Some(ref emitter) = self.emitter {
            emitter.emit_connection_status(connection_id, &conn_name, "connected");
        }

        Ok(pool)
    }

    pub async fn invalidate(&self, connection_id: &str) {
        let entry_name = {
            let mut pools = self.pools.write().await;
            pools.remove(connection_id).map(|e| e.name)
        };
        if let (Some(name), Some(ref emitter)) = (entry_name, &self.emitter) {
            emitter.emit_connection_status(connection_id, &name, "disconnected");
        }
    }

    pub async fn cleanup_expired(&self) {
        let to_check: Vec<(String, Instant, u64)> = {
            let pools = self.pools.read().await;
            pools
                .iter()
                .map(|(id, entry)| (id.clone(), entry.last_used, entry.ttl_seconds))
                .collect()
        };

        let mut to_remove = Vec::new();
        for (id, last_used, ttl_seconds) in &to_check {
            if last_used.elapsed() > Duration::from_secs(*ttl_seconds) {
                to_remove.push(id.clone());
            }
        }

        if !to_remove.is_empty() {
            let mut pools = self.pools.write().await;
            for id in &to_remove {
                if let Some(entry) = pools.remove(id) {
                    if let Some(ref emitter) = self.emitter {
                        emitter.emit_connection_status(id, &entry.name, "disconnected");
                    }
                }
            }
        }
    }

    /// Returns the connection's query_timeout_ms from the pool cache, if available.
    pub async fn get_query_timeout(&self, connection_id: &str) -> Option<u64> {
        let pools = self.pools.read().await;
        pools.get(connection_id).map(|e| e.query_timeout_ms)
    }

    /// Delete query_log entries older than 30 days or exceeding 50k rows.
    pub async fn cleanup_query_log(&self) {
        let _ = sqlx::query("DELETE FROM query_log WHERE created_at < datetime('now', '-30 days')")
            .execute(&self.app_pool)
            .await;

        // Cap at 50k entries — delete oldest beyond that
        let _ = sqlx::query(
            "DELETE FROM query_log WHERE id NOT IN (SELECT id FROM query_log ORDER BY id DESC LIMIT 50000)",
        )
        .execute(&self.app_pool)
        .await;
    }

    pub async fn test_connection(&self, connection_id: &str) -> Result<(), String> {
        let conn = self
            .lookup
            .lookup(connection_id)
            .await?
            .ok_or_else(|| format!("Connection '{}' not found", connection_id))?;

        let pool = create_pool(&conn).await?;
        pool.test().await?;

        // The legacy `UPDATE connections SET last_tested_at` write is
        // dropped: the file-backed schema doesn't carry that field, and
        // the live status emitter (`emit_connection_status`) is the
        // user-facing signal anyway. Reintroduce as a per-machine cache
        // (e.g. `~/.config/httui/connection_status.toml`) if a UI need
        // emerges — out of scope for v1 (audit-015 Phase 3 decision).

        Ok(())
    }

    /// Test-only seeding of cache entries — bypasses `create_pool` so
    /// cache methods (`invalidate`, `cleanup_expired`, `get_query_timeout`)
    /// can be unit-tested without standing up a live PG/MySQL pool.
    #[cfg(test)]
    pub(crate) async fn insert_for_test(
        &self,
        connection_id: &str,
        name: &str,
        pool: Arc<DatabasePool>,
        last_used: Instant,
        ttl_seconds: u64,
        query_timeout_ms: u64,
    ) {
        let mut pools = self.pools.write().await;
        pools.insert(
            connection_id.to_string(),
            PoolEntry {
                pool,
                name: name.to_string(),
                last_used,
                ttl_seconds,
                query_timeout_ms,
            },
        );
    }

    #[cfg(test)]
    pub(crate) async fn cache_size(&self) -> usize {
        self.pools.read().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    /// Minimal `ConnectionLookup` impl that always returns Ok(None) —
    /// stand-in for the cache-only tests where lookup isn't exercised.
    struct NoopLookup;

    #[async_trait::async_trait]
    impl ConnectionLookup for NoopLookup {
        async fn lookup(
            &self,
            _key: &str,
        ) -> Result<Option<crate::db::connections::Connection>, String> {
            Ok(None)
        }
    }

    /// Capturing emitter — counts the calls so `invalidate` /
    /// `cleanup_expired` can assert "we did emit once".
    struct CountingEmitter {
        calls: AtomicUsize,
    }

    impl StatusEmitter for CountingEmitter {
        fn emit_connection_status(&self, _: &str, _: &str, _: &str) {
            self.calls.fetch_add(1, Ordering::SeqCst);
        }
    }

    async fn memory_app_pool() -> SqlitePool {
        SqlitePool::connect("sqlite::memory:").await.unwrap()
    }

    async fn memory_target_pool() -> Arc<DatabasePool> {
        let p = SqlitePool::connect("sqlite::memory:").await.unwrap();
        Arc::new(DatabasePool::Sqlite(p))
    }

    #[tokio::test]
    async fn new_standalone_starts_with_empty_cache_and_no_emitter() {
        let app = memory_app_pool().await;
        let mgr = PoolManager::new_standalone(Arc::new(NoopLookup), app);
        assert_eq!(mgr.cache_size().await, 0);
        assert!(mgr.emitter.is_none());
    }

    #[tokio::test]
    async fn new_with_emitter_holds_emitter() {
        let app = memory_app_pool().await;
        let emitter = Arc::new(CountingEmitter {
            calls: AtomicUsize::new(0),
        });
        let mgr = PoolManager::new_with_emitter(Arc::new(NoopLookup), app, emitter);
        assert!(mgr.emitter.is_some());
    }

    #[tokio::test]
    async fn app_pool_returns_borrow_of_inner_pool() {
        let app = memory_app_pool().await;
        let mgr = PoolManager::new_standalone(Arc::new(NoopLookup), app);
        // Use it to run a query — proves we got a working pool back.
        let row: (i32,) = sqlx::query_as("SELECT 1")
            .fetch_one(mgr.app_pool())
            .await
            .unwrap();
        assert_eq!(row.0, 1);
    }

    #[tokio::test]
    async fn invalidate_removes_entry_and_emits_disconnected() {
        let app = memory_app_pool().await;
        let emitter = Arc::new(CountingEmitter {
            calls: AtomicUsize::new(0),
        });
        let mgr = PoolManager::new_with_emitter(Arc::new(NoopLookup), app, emitter.clone());

        let pool = memory_target_pool().await;
        mgr.insert_for_test("c1", "test-conn", pool, Instant::now(), 60, 30_000)
            .await;
        assert_eq!(mgr.cache_size().await, 1);

        mgr.invalidate("c1").await;
        assert_eq!(mgr.cache_size().await, 0);
        assert_eq!(emitter.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn invalidate_unknown_id_is_a_noop_no_emit() {
        let app = memory_app_pool().await;
        let emitter = Arc::new(CountingEmitter {
            calls: AtomicUsize::new(0),
        });
        let mgr = PoolManager::new_with_emitter(Arc::new(NoopLookup), app, emitter.clone());
        mgr.invalidate("nope").await;
        assert_eq!(emitter.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn invalidate_without_emitter_drops_silently() {
        let app = memory_app_pool().await;
        let mgr = PoolManager::new_standalone(Arc::new(NoopLookup), app);
        let pool = memory_target_pool().await;
        mgr.insert_for_test("c1", "x", pool, Instant::now(), 60, 30_000)
            .await;
        mgr.invalidate("c1").await;
        assert_eq!(mgr.cache_size().await, 0);
    }

    #[tokio::test]
    async fn cleanup_expired_drops_entries_past_ttl_and_emits_each() {
        let app = memory_app_pool().await;
        let emitter = Arc::new(CountingEmitter {
            calls: AtomicUsize::new(0),
        });
        let mgr = PoolManager::new_with_emitter(Arc::new(NoopLookup), app, emitter.clone());

        let stale = Instant::now() - Duration::from_secs(120);
        let fresh = Instant::now();
        let p1 = memory_target_pool().await;
        let p2 = memory_target_pool().await;
        let p3 = memory_target_pool().await;

        mgr.insert_for_test("expired-1", "a", p1, stale, 60, 30_000).await;
        mgr.insert_for_test("expired-2", "b", p2, stale, 60, 30_000).await;
        mgr.insert_for_test("fresh", "c", p3, fresh, 60, 30_000).await;
        assert_eq!(mgr.cache_size().await, 3);

        mgr.cleanup_expired().await;

        assert_eq!(mgr.cache_size().await, 1);
        assert_eq!(emitter.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn cleanup_expired_with_only_fresh_entries_is_a_noop() {
        let app = memory_app_pool().await;
        let mgr = PoolManager::new_standalone(Arc::new(NoopLookup), app);
        let pool = memory_target_pool().await;
        mgr.insert_for_test("fresh", "x", pool, Instant::now(), 300, 30_000)
            .await;
        mgr.cleanup_expired().await;
        assert_eq!(mgr.cache_size().await, 1);
    }

    #[tokio::test]
    async fn get_query_timeout_returns_cached_value_or_none() {
        let app = memory_app_pool().await;
        let mgr = PoolManager::new_standalone(Arc::new(NoopLookup), app);
        let pool = memory_target_pool().await;
        mgr.insert_for_test("c1", "x", pool, Instant::now(), 60, 12_345)
            .await;
        assert_eq!(mgr.get_query_timeout("c1").await, Some(12_345));
        assert_eq!(mgr.get_query_timeout("missing").await, None);
    }

    #[tokio::test]
    async fn get_pool_returns_not_found_when_lookup_returns_none() {
        let app = memory_app_pool().await;
        let mgr = PoolManager::new_standalone(Arc::new(NoopLookup), app);
        match mgr.get_pool("missing").await {
            Ok(_) => panic!("expected not-found error"),
            Err(e) => assert!(e.contains("not found"), "got: {e}"),
        }
    }

    #[tokio::test]
    async fn get_pool_serves_cached_entry_without_calling_lookup() {
        let app = memory_app_pool().await;
        // NoopLookup would error if called; serving from cache must
        // bypass it entirely.
        let mgr = PoolManager::new_standalone(Arc::new(NoopLookup), app);
        let pool = memory_target_pool().await;
        mgr.insert_for_test("c1", "x", pool.clone(), Instant::now(), 60, 30_000)
            .await;
        let returned = mgr.get_pool("c1").await.unwrap();
        // Same Arc as inserted
        assert!(Arc::ptr_eq(&returned, &pool));
    }

    #[tokio::test]
    async fn cleanup_query_log_drops_old_rows_and_caps_recent() {
        let app = memory_app_pool().await;
        sqlx::query(
            r#"CREATE TABLE query_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )"#,
        )
        .execute(&app)
        .await
        .unwrap();
        // 5 old (>30 days) + 5 recent rows
        for _ in 0..5 {
            sqlx::query("INSERT INTO query_log (created_at) VALUES (datetime('now', '-60 days'))")
                .execute(&app)
                .await
                .unwrap();
        }
        for _ in 0..5 {
            sqlx::query("INSERT INTO query_log (created_at) VALUES (datetime('now'))")
                .execute(&app)
                .await
                .unwrap();
        }
        let mgr = PoolManager::new_standalone(Arc::new(NoopLookup), app.clone());
        mgr.cleanup_query_log().await;

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM query_log")
            .fetch_one(&app)
            .await
            .unwrap();
        assert_eq!(count, 5, "old rows should be deleted, recent ones kept");
    }

    // ───── V11 cenário 2: host:port session override ─────

    #[test]
    fn host_port_override_is_empty_only_when_both_none() {
        assert!(HostPortOverride::default().is_empty());
        assert!(!HostPortOverride {
            host: Some("db".into()),
            port: None,
        }
        .is_empty());
        assert!(!HostPortOverride {
            host: None,
            port: Some(5433),
        }
        .is_empty());
    }

    #[test]
    fn host_port_override_cache_suffix_is_stable_and_distinct() {
        let a = HostPortOverride {
            host: Some("db.internal".into()),
            port: Some(5433),
        };
        assert_eq!(a.cache_suffix(), "#ovr:db.internal:5433");
        // Distinct pairs → distinct suffix; missing fields render as "-".
        assert_eq!(
            HostPortOverride {
                host: None,
                port: Some(6000)
            }
            .cache_suffix(),
            "#ovr:-:6000"
        );
        assert_ne!(a.cache_suffix(), HostPortOverride::default().cache_suffix());
    }

    /// App pool + connections table + one sqlite connection record,
    /// wired through the production `SqliteLookup` so `create_pool`
    /// actually runs (sqlite ignores host/port — we're asserting the
    /// manager's cache-keying / fallback, not a live PG reconnect).
    async fn sqlite_lookup_env() -> (PoolManager, String) {
        let app = memory_app_pool().await;
        sqlx::query(
            r#"CREATE TABLE connections (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                driver TEXT NOT NULL CHECK (driver IN ('postgres','mysql','sqlite')),
                host TEXT, port INTEGER, database_name TEXT,
                username TEXT, password TEXT,
                ssl_mode TEXT DEFAULT 'disable',
                timeout_ms INTEGER DEFAULT 10000,
                query_timeout_ms INTEGER DEFAULT 30000,
                ttl_seconds INTEGER DEFAULT 300,
                max_pool_size INTEGER DEFAULT 5,
                is_readonly INTEGER NOT NULL DEFAULT 0,
                last_tested_at TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )"#,
        )
        .execute(&app)
        .await
        .unwrap();
        let conn = crate::db::connections::create_connection(
            &app,
            crate::db::connections::CreateConnection {
                name: "ovr-sqlite".to_string(),
                driver: "sqlite".to_string(),
                host: None,
                port: None,
                database_name: Some(":memory:".to_string()),
                username: None,
                password: None,
                ssl_mode: None,
                timeout_ms: None,
                query_timeout_ms: None,
                ttl_seconds: None,
                max_pool_size: None,
                is_readonly: None,
            },
        )
        .await
        .unwrap();
        let mgr =
            PoolManager::new_standalone(crate::db::lookup::SqliteLookup::new(app.clone()), app);
        (mgr, conn.id)
    }

    #[tokio::test]
    async fn override_none_or_empty_takes_the_base_pool_path() {
        let (mgr, id) = sqlite_lookup_env().await;
        let base = mgr.get_pool_with_override(&id, None).await.unwrap();
        // Empty override resolves to the SAME base entry, not a new one.
        let empty = HostPortOverride::default();
        let again = mgr.get_pool_with_override(&id, Some(&empty)).await.unwrap();
        assert!(Arc::ptr_eq(&base, &again));
        assert_eq!(mgr.cache_size().await, 1);
    }

    #[tokio::test]
    async fn override_creates_a_separate_pool_keeping_base_clean() {
        let (mgr, id) = sqlite_lookup_env().await;
        let base = mgr.get_pool(&id).await.unwrap();
        let ov = HostPortOverride {
            host: Some("db.staging".into()),
            port: Some(5599),
        };
        let overridden = mgr.get_pool_with_override(&id, Some(&ov)).await.unwrap();
        // Distinct pools, two distinct cache entries.
        assert!(!Arc::ptr_eq(&base, &overridden));
        assert_eq!(mgr.cache_size().await, 2);
        // Base entry untouched; clearing the override (None) returns it.
        let back = mgr.get_pool_with_override(&id, None).await.unwrap();
        assert!(Arc::ptr_eq(&base, &back));
    }

    #[tokio::test]
    async fn same_override_reuses_its_cached_pool() {
        let (mgr, id) = sqlite_lookup_env().await;
        let ov = HostPortOverride {
            host: Some("db.staging".into()),
            port: None,
        };
        let first = mgr.get_pool_with_override(&id, Some(&ov)).await.unwrap();
        let second = mgr.get_pool_with_override(&id, Some(&ov)).await.unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(mgr.cache_size().await, 1);
    }

    /// Same as `sqlite_lookup_env` but wired with a counting emitter so
    /// the "connected" emission branch in `get_pool_keyed` is covered.
    async fn sqlite_lookup_env_with_emitter() -> (PoolManager, String, Arc<CountingEmitter>) {
        let (mgr, id) = sqlite_lookup_env().await;
        // Rebuild with the same lookup wiring + an emitter.
        let app = mgr.app_pool().clone();
        let emitter = Arc::new(CountingEmitter {
            calls: AtomicUsize::new(0),
        });
        let mgr = PoolManager::new_with_emitter(
            crate::db::lookup::SqliteLookup::new(app.clone()),
            app,
            emitter.clone(),
        );
        (mgr, id, emitter)
    }

    #[tokio::test]
    async fn get_pool_emits_connected_for_base_and_override() {
        let (mgr, id, emitter) = sqlite_lookup_env_with_emitter().await;
        let _base = mgr.get_pool(&id).await.unwrap();
        let ov = HostPortOverride {
            host: Some("db.staging".into()),
            port: Some(5599),
        };
        let _ovr = mgr.get_pool_with_override(&id, Some(&ov)).await.unwrap();
        // One emit for the base pool, one for the override-keyed pool.
        assert_eq!(emitter.calls.load(Ordering::SeqCst), 2);
        assert_eq!(mgr.cache_size().await, 2);
    }

    #[tokio::test]
    async fn get_pool_with_override_propagates_not_found() {
        let app = memory_app_pool().await;
        let mgr = PoolManager::new_standalone(Arc::new(NoopLookup), app);
        let ov = HostPortOverride {
            host: Some("h".into()),
            port: None,
        };
        match mgr.get_pool_with_override("missing", Some(&ov)).await {
            Ok(_) => panic!("expected not-found error"),
            Err(e) => assert!(e.contains("not found"), "got: {e}"),
        }
    }
}
