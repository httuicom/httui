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

    /// Refresh the connection_id → name cache from SQLite. Call
    /// after creating / renaming / deleting a connection so block
    /// footers update without restarting the TUI.
    #[allow(dead_code)] // wired up by the upcoming connection picker.
    pub fn refresh_connection_names(&mut self) {
        self.connection_names = load_connection_names(self.pool_manager.app_pool());
    }

    /// Re-resolve the active environment's display name from the
    /// SQLite registry and stash it on `active_env_name`. Cheap to
    /// call (single async query under `block_in_place`) — invoke
    /// after a hypothetical env-switch so the status bar chip
    /// updates without a TUI restart. Today no UI mutates the
    /// active env, so this only runs at startup.
    pub fn refresh_active_env_name(&mut self) {
        let pool = self.pool_manager.app_pool().clone();
        self.active_env_name = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let id = httui_core::db::environments::get_active_environment_id(&pool).await?;
                let envs = httui_core::db::environments::list_environments(&pool)
                    .await
                    .ok()?;
                envs.into_iter().find(|e| e.id == id).map(|e| e.name)
            })
        });
    }
}
