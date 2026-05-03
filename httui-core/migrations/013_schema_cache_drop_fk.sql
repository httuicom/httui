-- V4 fix: connections moved to .httui/connections.toml (Epic 12)
-- but the schema_cache.connection_id FK still points at the legacy
-- SQLite `connections` table. Inserts fail with code 787 the moment
-- a file-backed connection (e.g. SQLite added via the new
-- ConnectionsPage) introspects.
--
-- Drop the FK by recreating the table without it. Cascading delete
-- on connection drop now happens via explicit cleanup at the
-- connections_store delete path (Tauri command invalidates pool +
-- schema_cache by name).

CREATE TABLE schema_cache_new (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    connection_id TEXT NOT NULL,
    table_name    TEXT NOT NULL,
    column_name   TEXT NOT NULL,
    data_type     TEXT,
    cached_at     TEXT NOT NULL DEFAULT (datetime('now')),
    schema_name   TEXT
);

INSERT INTO schema_cache_new
    (id, connection_id, table_name, column_name, data_type, cached_at, schema_name)
SELECT id, connection_id, table_name, column_name, data_type, cached_at, schema_name
FROM schema_cache;

DROP TABLE schema_cache;
ALTER TABLE schema_cache_new RENAME TO schema_cache;

-- Recreate the migration-006 unique index after the table swap.
CREATE UNIQUE INDEX IF NOT EXISTS schema_cache_unique
    ON schema_cache(connection_id, schema_name, table_name, column_name);
