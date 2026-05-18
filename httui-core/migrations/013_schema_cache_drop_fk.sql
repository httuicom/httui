-- fix: connections moved to.httui/connections.toml
-- but the schema_cache.connection_id FK still points at the legacy
-- SQLite `connections` table. Inserts fail with code 787 the moment
-- a file-backed connection (e.g. SQLite added via the new
-- ConnectionsPage) introspects.
--
-- Drop the FK by recreating the table without it. Cascading delete
-- on connection drop now happens via explicit cleanup at the
-- connections_store delete path (Tauri command invalidates pool +
-- schema_cache by name).

DROP INDEX IF EXISTS schema_cache_unique;
ALTER TABLE schema_cache RENAME TO schema_cache_old;

CREATE TABLE schema_cache (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    connection_id TEXT NOT NULL,
    table_name    TEXT NOT NULL,
    column_name   TEXT NOT NULL,
    data_type     TEXT,
    cached_at     TEXT NOT NULL DEFAULT (datetime('now')),
    schema_name   TEXT
);

INSERT INTO schema_cache
    (id, connection_id, table_name, column_name, data_type, cached_at, schema_name)
SELECT id, connection_id, table_name, column_name, data_type, cached_at, schema_name
FROM schema_cache_old;

DROP TABLE schema_cache_old;

CREATE UNIQUE INDEX IF NOT EXISTS schema_cache_unique
    ON schema_cache(connection_id, schema_name, table_name, column_name);
