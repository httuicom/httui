-- 014_block_results_alias.sql
--
-- Add an `alias` column to block_results so the TUI ref completion
-- popup (and the BLOCKS view footer / autocomplete cache hydrate) can
-- look up the latest persisted response by `(file_path, alias)`
-- instead of `(file_path, hash)`. Hash-based lookups stay correct for
-- "is this exact request cached?" but they miss when another pane
-- (or session) re-ran the same alias with a different shape (added a
-- header, edited a param). With `alias` we can resolve
-- `{{alias.response.body.…}}` to the most recent response that block
-- produced, regardless of which precise hash it landed on.
--
-- The column is nullable: pre-existing rows wrote responses without an
-- alias (no way to backfill — the alias isn't in the row), and HTTP /
-- DB blocks without an `alias=` fence still persist the run.

ALTER TABLE block_results ADD COLUMN alias TEXT NULL DEFAULT NULL;

CREATE INDEX IF NOT EXISTS idx_block_results_file_alias_time
    ON block_results(file_path, alias, executed_at DESC);
