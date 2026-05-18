//! In-memory schema cache for the TUI.
//!
//! Mirrors the desktop's `useSchemaCacheStore` (`src/stores/schemaCache.ts`):
//! `httui-core::db::schema_cache` already gives us a SQLite-backed
//! introspection cache (TTL 300s). On top of that we keep a per-`App`
//! in-memory cache so completion popups and other UI can read schema
//! synchronously inside a single frame.
//!
//! The fetch pipeline is one-direction:
//!   `App::ensure_schema_loaded(conn_id)`
//!     → spawns `tokio::task` if not already pending
//!     → task tries `get_cached_schema` (SQLite cache, fast path)
//!     → falls back to `introspect_schema` (driver query, slow path)
//!     → emits `AppEvent::SchemaLoaded { connection_id, result }`
//!     → main loop folds the result back via `App::on_schema_loaded`
//!
//! Dedup is naive on purpose: the `pending` set lives on the main
//! thread (we never call `ensure_schema_loaded` off it) so a
//! `HashSet<String>` protected by nothing more than ownership is
//! enough. Concurrent fetches of the same connection collapse to a
//! single in-flight task.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use httui_core::db::schema_cache::SchemaEntry;

/// One database schema, grouped by `(schema, table)` and ready to
/// hand to the SQL completion engine.
///
/// The `name`/`data_type` fields read as dead code today; b
/// (SQL completion source) is the consumer that lights them up.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SchemaTable {
    /// Qualifying namespace — `Some(name)` on Postgres / MySQL,
    /// `None` on SQLite. Two tables with the same name in different
    /// schemas don't collide because we key on `(schema, name)`.
    pub schema: Option<String>,
    pub name: String,
    pub columns: Vec<SchemaColumn>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SchemaColumn {
    pub name: String,
    pub data_type: Option<String>,
}

/// Cache slot for one connection. `fetched_at` lets a future
/// `:schema refresh` decide whether to bypass on TTL — for now the
/// SQLite cache layer (`get_cached_schema`) is the source of truth
/// for staleness.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SchemaCacheEntry {
    pub tables: Vec<SchemaTable>,
    pub fetched_at: Instant,
}

/// In-memory cache + dedup state. Owned by `App`. The pending set is
/// the only piece that gates concurrent fetches: a connection in
/// `pending` means a `tokio::spawn` is already running for it and the
/// caller should wait for `AppEvent::SchemaLoaded` instead of spawning
/// a second task.
#[derive(Debug, Default)]
pub struct SchemaCache {
    by_connection: HashMap<String, SchemaCacheEntry>,
    pending: HashSet<String>,
}

impl SchemaCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sync read for the renderer / completion engine. `None` means
    /// "not loaded yet" — typically the caller will trigger a fetch
    /// via `App::ensure_schema_loaded` and re-render when the
    /// `SchemaLoaded` event fires.
    pub fn get(&self, connection_id: &str) -> Option<&SchemaCacheEntry> {
        self.by_connection.get(connection_id)
    }

    /// Has a fetch already been kicked off for this connection?
    /// Used by `ensure_schema_loaded` to dedup spawns.
    pub fn is_pending(&self, connection_id: &str) -> bool {
        self.pending.contains(connection_id)
    }

    /// Mark a connection as fetching. Called when the spawn fires;
    /// cleared when the result arrives.
    pub fn mark_pending(&mut self, connection_id: &str) {
        self.pending.insert(connection_id.to_string());
    }

    /// Fold a successful introspection into the cache. Replaces any
    /// previous entry — the SQLite layer below us already enforced
    /// TTL, so by the time we land here the data is fresh.
    pub fn store(&mut self, connection_id: &str, tables: Vec<SchemaTable>) {
        self.by_connection.insert(
            connection_id.to_string(),
            SchemaCacheEntry {
                tables,
                fetched_at: Instant::now(),
            },
        );
    }

    /// Clear the pending flag without storing anything. Used when an
    /// introspection fails — we surface the error in the status bar
    /// but don't poison the cache, so a later retry can succeed.
    pub fn clear_pending(&mut self, connection_id: &str) {
        self.pending.remove(connection_id);
    }

    /// Drop a connection's cached schema. For when the user deletes
    /// the connection or triggers an explicit refresh.
    #[allow(dead_code)]
    pub fn invalidate(&mut self, connection_id: &str) {
        self.by_connection.remove(connection_id);
        self.pending.remove(connection_id);
    }
}

/// Group flat `SchemaEntry` rows into `Vec<SchemaTable>`, keyed by
/// `(schema, table)` so `public.users` and `auth.users` don't merge.
/// Pure function — easy to unit-test without a live DB.
pub fn group_entries(entries: Vec<SchemaEntry>) -> Vec<SchemaTable> {
    // Order of insertion is preserved by `IndexMap`-style iteration in
    // a `Vec<(key, table)>` accumulator; we sort at the end so the
    // output is deterministic regardless of input order. Schema-less
    // rows (SQLite) come first to match the desktop's empty-schema
    // sort key (`""`).
    let mut by_key: Vec<((Option<String>, String), SchemaTable)> = Vec::new();
    for entry in entries {
        let key = (entry.schema_name.clone(), entry.table_name.clone());
        if let Some(slot) = by_key.iter_mut().find(|(k, _)| *k == key) {
            slot.1.columns.push(SchemaColumn {
                name: entry.column_name,
                data_type: entry.data_type,
            });
        } else {
            by_key.push((
                key,
                SchemaTable {
                    schema: entry.schema_name,
                    name: entry.table_name,
                    columns: vec![SchemaColumn {
                        name: entry.column_name,
                        data_type: entry.data_type,
                    }],
                },
            ));
        }
    }
    by_key.sort_by(|(ka, _), (kb, _)| {
        let sa = ka.0.as_deref().unwrap_or("");
        let sb = kb.0.as_deref().unwrap_or("");
        sa.cmp(sb).then_with(|| ka.1.cmp(&kb.1))
    });
    by_key.into_iter().map(|(_, t)| t).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(schema: Option<&str>, table: &str, col: &str, ty: Option<&str>) -> SchemaEntry {
        SchemaEntry {
            schema_name: schema.map(String::from),
            table_name: table.to_string(),
            column_name: col.to_string(),
            data_type: ty.map(String::from),
        }
    }

    #[test]
    fn group_entries_collapses_columns_per_table() {
        // Two columns under one table → one SchemaTable with two columns.
        let entries = vec![
            entry(None, "users", "id", Some("INTEGER")),
            entry(None, "users", "name", Some("TEXT")),
        ];
        let tables = group_entries(entries);
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "users");
        assert_eq!(tables[0].columns.len(), 2);
        assert_eq!(tables[0].columns[0].name, "id");
        assert_eq!(tables[0].columns[1].name, "name");
    }

    #[test]
    fn group_entries_separates_same_table_across_schemas() {
        // `public.users` vs `auth.users` must NOT merge — desktop has
        // a regression test for this exact case (different schemas
        // with identical table names).
        let entries = vec![
            entry(Some("public"), "users", "id", None),
            entry(Some("auth"), "users", "uid", None),
        ];
        let tables = group_entries(entries);
        assert_eq!(tables.len(), 2);
        // Sorted by schema → "auth" before "public".
        assert_eq!(tables[0].schema.as_deref(), Some("auth"));
        assert_eq!(tables[1].schema.as_deref(), Some("public"));
    }

    #[test]
    fn group_entries_sorts_schemaless_first() {
        // SQLite rows arrive with `schema_name: None` and should sort
        // ahead of namespaced rows so they cluster cleanly at the top
        // of the schema panel (matches desktop's `""` sort key).
        let entries = vec![
            entry(Some("public"), "z_table", "col", None),
            entry(None, "a_table", "col", None),
        ];
        let tables = group_entries(entries);
        assert_eq!(tables[0].schema, None);
        assert_eq!(tables[0].name, "a_table");
        assert_eq!(tables[1].schema.as_deref(), Some("public"));
    }

    #[test]
    fn store_replaces_existing_entry() {
        // A second introspection for the same connection should
        // overwrite — useful after a manual `:schema refresh`.
        let mut cache = SchemaCache::new();
        cache.store(
            "conn-1",
            vec![SchemaTable {
                schema: None,
                name: "old".into(),
                columns: vec![],
            }],
        );
        cache.store(
            "conn-1",
            vec![SchemaTable {
                schema: None,
                name: "new".into(),
                columns: vec![],
            }],
        );
        let got = cache.get("conn-1").expect("entry exists");
        assert_eq!(got.tables.len(), 1);
        assert_eq!(got.tables[0].name, "new");
    }

    #[test]
    fn pending_dedup_lets_only_one_fetch_through() {
        // The first `mark_pending` reserves the slot; subsequent
        // `is_pending` calls return true so a second `tokio::spawn`
        // won't fire. `clear_pending` releases it for retries.
        let mut cache = SchemaCache::new();
        assert!(!cache.is_pending("conn-1"));
        cache.mark_pending("conn-1");
        assert!(cache.is_pending("conn-1"));
        cache.clear_pending("conn-1");
        assert!(!cache.is_pending("conn-1"));
    }

    #[test]
    fn invalidate_drops_both_data_and_pending_flag() {
        // After a connection is deleted we don't want a stale schema
        // cluttering the cache or a stuck pending flag if the fetch
        // raced.
        let mut cache = SchemaCache::new();
        cache.store(
            "conn-1",
            vec![SchemaTable {
                schema: None,
                name: "users".into(),
                columns: vec![],
            }],
        );
        cache.mark_pending("conn-1");
        cache.invalidate("conn-1");
        assert!(cache.get("conn-1").is_none());
        assert!(!cache.is_pending("conn-1"));
    }
}
