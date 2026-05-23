//! `impl App` — schema / connection-name / active-env cache refresh.
//!
//! Mechanically extracted from the monolithic `impl App` in `app.rs`
//! (tui-v2 vertical 1, fase 2 p2-schema) — pure code move, no behavior
//! change. Sibling `impl App {}` block; methods stay `pub fn` so every
//! `app.foo()` call site keeps resolving unchanged.

use super::{load_connection_names, App, StatusKind};

impl App {
    /// Kick off a background introspection of `connection_id` if one
    /// isn't already pending and the cache is empty. Cheap to call
    /// repeatedly — the dedup gate makes the second/third call a
    /// no-op. Result lands as `AppEvent::SchemaLoaded`.
    pub fn ensure_schema_loaded(&mut self, connection_id: &str) {
        if self.schema_cache.get(connection_id).is_some() {
            return;
        }
        if self.schema_cache.is_pending(connection_id) {
            return;
        }
        let Some(sender) = self.event_sender.clone() else {
            // No event loop wired yet (unit-test-only path). Skip
            // silently — the test that constructed the App didn't
            // need async cache resolution.
            return;
        };
        self.schema_cache.mark_pending(connection_id);
        let pool_mgr = self.pool_manager.clone();
        let app_pool = self.pool_manager.app_pool().clone();
        let conn_id = connection_id.to_string();
        tokio::spawn(async move {
            // SQLite cache (TTL 300s) is the fast path; introspection
            // hits the actual driver only on miss / expired entries.
            // Mirrors `useSchemaCacheStore.ensureLoaded` on desktop.
            let result =
                match httui_core::db::schema_cache::get_cached_schema(&app_pool, &conn_id, 300)
                    .await
                {
                    Ok(Some(entries)) if !entries.is_empty() => Ok(entries),
                    _ => {
                        httui_core::db::schema_cache::introspect_schema(
                            &pool_mgr, &app_pool, &conn_id,
                        )
                        .await
                    }
                };
            let _ = sender.send(crate::event::AppEvent::SchemaLoaded {
                connection_id: conn_id,
                result,
            });
        });
    }

    /// Fold a `SchemaLoaded` event into `schema_cache`. Called from
    /// the main loop. Errors surface in the status bar but don't
    /// poison the cache so a retry can succeed.
    pub fn on_schema_loaded(
        &mut self,
        connection_id: String,
        result: Result<Vec<httui_core::db::schema_cache::SchemaEntry>, String>,
    ) {
        self.schema_cache.clear_pending(&connection_id);
        match result {
            Ok(entries) => {
                let tables = crate::schema::group_entries(entries);
                self.schema_cache.store(&connection_id, tables);
            }
            Err(msg) => {
                self.set_status(
                    StatusKind::Error,
                    format!("schema introspection failed: {msg}"),
                );
            }
        }
    }

    /// Refresh the connection name cache from the vault's
    /// `connections.toml`. Call after creating / renaming / deleting
    /// a connection so block footers update without restarting the
    /// TUI.
    #[allow(dead_code)] // wired up by the connection picker.
    pub fn refresh_connection_names(&mut self) {
        self.connection_names = load_connection_names(&self.connections_store);
    }

    /// Re-resolve the active environment's display name from the
    /// SQLite registry and stash it on `active_env_name`. Cheap to
    /// call (single async query under `block_in_place`) — invoke
    /// after a hypothetical env-switch so the status bar chip
    /// updates without a TUI restart. Today no UI mutates the
    /// active env, so this only runs at startup.
    /// V4 P1 (2026-05-23): now reads `<vault>/envs/` + `user.toml`
    /// via `EnvironmentsStore`. The TOML store keys envs by name so
    /// the previous id-to-name lookup collapses into a single read.
    pub fn refresh_active_env_name(&mut self) {
        let store = self.environments_store.clone();
        self.active_env_name = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async move { store.active_env().await.ok().flatten() })
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::event::AppEvent;
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use sqlx::SqlitePool;
    use tempfile::TempDir;

    /// Build an `App` over a fresh vault + isolated pool. `App::new`
    /// calls `block_in_place` → multi-thread runtime required. Returns
    /// the pool clone too so a test can seed rows on the same DB.
    async fn app_fixture() -> (App, SqlitePool, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        std::fs::write(vault.path().join("note.md"), "body\n").unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let app = App::new(Config::default(), resolved, pool.clone());
        (app, pool, data, vault)
    }

    async fn seed_connection(app: &App, name: &str) {
        use httui_core::vault_config::CreateConnectionInput;
        app.connections_store
            .create(CreateConnectionInput {
                name: name.into(),
                driver: "postgres".into(),
                host: Some("localhost".into()),
                port: Some(5432),
                database_name: Some("db".into()),
                username: Some("user".into()),
                password: None,
                ssl_mode: None,
                is_readonly: None,
                description: None,
            })
            .await
            .unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ensure_schema_loaded_noop_when_already_cached() {
        let (mut app, _p, _d, _v) = app_fixture().await;
        app.schema_cache.store("conn-1", Vec::new());
        // Cached → returns early, never marks pending.
        app.ensure_schema_loaded("conn-1");
        assert!(!app.schema_cache.is_pending("conn-1"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ensure_schema_loaded_noop_when_already_pending() {
        let (mut app, _p, _d, _v) = app_fixture().await;
        app.schema_cache.mark_pending("conn-2");
        app.ensure_schema_loaded("conn-2");
        // Still pending, no panic, no second spawn.
        assert!(app.schema_cache.is_pending("conn-2"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ensure_schema_loaded_noop_without_event_sender() {
        let (mut app, _p, _d, _v) = app_fixture().await;
        // No event loop wired (unit-test path) → silent skip, the
        // connection never gets marked pending.
        assert!(app.event_sender.is_none());
        app.ensure_schema_loaded("conn-3");
        assert!(!app.schema_cache.is_pending("conn-3"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ensure_schema_loaded_spawns_and_emits_schema_loaded_event() {
        let (mut app, _p, _d, _v) = app_fixture().await;
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        app.event_sender = Some(tx);

        app.ensure_schema_loaded("missing-conn");
        // The connection is marked pending the moment the spawn fires.
        assert!(app.schema_cache.is_pending("missing-conn"));

        // The background task resolves (introspection fails for an
        // unknown connection) and pushes a `SchemaLoaded` event.
        let evt = rx.recv().await.expect("SchemaLoaded event");
        match evt {
            AppEvent::SchemaLoaded {
                connection_id,
                result,
            } => {
                assert_eq!(connection_id, "missing-conn");
                assert!(result.is_err());
            }
            other => panic!("expected SchemaLoaded, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn on_schema_loaded_ok_stores_grouped_tables_and_clears_pending() {
        let (mut app, _p, _d, _v) = app_fixture().await;
        app.schema_cache.mark_pending("c");
        let entries = vec![httui_core::db::schema_cache::SchemaEntry {
            schema_name: None,
            table_name: "users".into(),
            column_name: "id".into(),
            data_type: Some("INTEGER".into()),
        }];
        app.on_schema_loaded("c".into(), Ok(entries));
        assert!(!app.schema_cache.is_pending("c"));
        let cached = app.schema_cache.get("c").expect("cached schema");
        assert_eq!(cached.tables.len(), 1);
        assert_eq!(cached.tables[0].name, "users");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn on_schema_loaded_err_sets_status_and_clears_pending() {
        let (mut app, _p, _d, _v) = app_fixture().await;
        app.schema_cache.mark_pending("bad");
        app.on_schema_loaded("bad".into(), Err("boom".into()));
        assert!(!app.schema_cache.is_pending("bad"));
        // Cache not poisoned — retry remains possible.
        assert!(app.schema_cache.get("bad").is_none());
        let msg = app.status_message.as_ref().expect("status set");
        assert_eq!(msg.kind, StatusKind::Error);
        assert!(msg.text.contains("schema introspection failed"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn refresh_connection_names_reloads_from_vault_toml() {
        let (mut app, _pool, _d, _v) = app_fixture().await;
        // Empty at startup (no connections seeded yet).
        assert!(app.connection_names.is_empty());
        seed_connection(&app, "prod-db").await;
        app.refresh_connection_names();
        assert_eq!(
            app.connection_names.get("prod-db").map(String::as_str),
            Some("prod-db")
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn refresh_active_env_name_resolves_active_environment() {
        let (mut app, _pool, _d, _v) = app_fixture().await;
        // No active env at startup.
        assert_eq!(app.active_env_name, None);
        app.environments_store
            .create_env("staging")
            .await
            .unwrap();
        app.environments_store
            .set_active_env(Some("staging"))
            .await
            .unwrap();
        app.refresh_active_env_name();
        assert_eq!(app.active_env_name.as_deref(), Some("staging"));
    }
}
