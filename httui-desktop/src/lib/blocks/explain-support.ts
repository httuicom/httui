// Explain-support lookup mirroring `httui-core::explain::prefix`.
//
// Pairs with the backend `prefix_explain_sql` function: the UI uses
// this synchronously to decide whether to show the EXPLAIN toggle
// and to render the "EXPLAIN unavailable for this driver" hint
// without an IPC roundtrip.
//
// **Drift contract:** when adding a driver to Rust's `normalize_driver`
// in `httui-core/src/explain/prefix.rs`, mirror it here. The list
// stays small enough that two sources are cheaper than a Tauri
// roundtrip on every UI render.

/** Driver strings that the backend can prefix with EXPLAIN. Lowercased,
 *  trimmed at lookup time so the consumer can pass raw connection
 *  config values. */
export const EXPLAIN_SUPPORTED_DRIVERS: ReadonlySet<string> = new Set([
  "postgres",
  "postgresql",
  "pg",
  "mysql",
  "mariadb",
]);

/** Body cap mirroring `httui-core::explain::prefix::EXPLAIN_BODY_CAP`.
 *  Used by the run-history viewer to flag "plan truncated" when the
 *  stored payload sits exactly at this size. */
export const EXPLAIN_BODY_CAP = 200_000;

/** True when the driver's EXPLAIN output is something the backend can
 *  capture as a JSON plan (Postgres / MySQL family). False for
 *  SQLite / BigQuery / Snowflake / Mongo / unknown — those map to
 *  `ExplainError::Unsupported` in the backend. */
export function driverSupportsExplain(
  driver: string | null | undefined,
): boolean {
  if (!driver) return false;
  return EXPLAIN_SUPPORTED_DRIVERS.has(driver.trim().toLowerCase());
}
