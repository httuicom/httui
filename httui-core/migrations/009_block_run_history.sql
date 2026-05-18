-- Block run history (HTTP block).
--
-- Stores metadata only (method, URL canonical, status, sizes, elapsed,
-- timestamp) for the most recent N runs per (file_path, alias). Body of
-- request and response are NEVER persisted here — privacy-by-default.
-- Use "Save as example" (separate feature) when full body retention is
-- the explicit intent.

CREATE TABLE IF NOT EXISTS block_run_history (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path     TEXT NOT NULL,
    block_alias   TEXT NOT NULL,
    method        TEXT NOT NULL,
    url_canonical TEXT NOT NULL,
    status        INTEGER,
    request_size  INTEGER,
    response_size INTEGER,
    elapsed_ms    INTEGER,
    outcome       TEXT NOT NULL,  -- success | error | cancelled
    ran_at        TEXT NOT NULL   -- ISO-8601 UTC
);

CREATE INDEX IF NOT EXISTS idx_brh_block
    ON block_run_history(file_path, block_alias, ran_at DESC);
