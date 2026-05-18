-- `EXPLAIN ANALYZE` plan blob attached to a run.
--
-- When the SQL block carried `explain=true` in its info-string, the
-- executor ran `EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) <sql>` (or
-- the per-driver equivalent — see `httui-core::explain::prefix_explain_sql`)
-- and stores the resulting JSON plan here. Capped + truncated by
-- `cap_explain_body` (200 KB). NULL when the run was a regular
-- query without EXPLAIN.

ALTER TABLE block_run_history ADD COLUMN plan TEXT
