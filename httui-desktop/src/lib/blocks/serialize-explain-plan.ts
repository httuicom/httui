// explain plan ↔ run-history serialization.
//
// `DbResponse.plan` (httui-core) hands the frontend a JSON value
// when `explain=true`: an object/array on Postgres (parsed JSON)
// or a string when the executor's 200 KB cap kicked in. The
// run-history table column is `plan TEXT`, so the consumer needs
// to flatten back to a string before calling
// `insert_block_history`.
//
// The Rust extractor already JSON-encodes everything before the
// cap is applied (`parsed.to_string()` happens inside
// `extract_plan_from_results`), but the wire shape can still be
// either:
//   - `serde_json::Value::String(capped)` when truncated, or
//   - `serde_json::Value::Object`/`Array` when within the cap.
//
// This helper handles both shapes plus the absent-plan case (no
// `explain=true` → `null`/`undefined` → no insert.plan field).

/** Serialize `DbResponse.plan` into the wire shape consumed by
 *  `InsertHistoryEntry.plan`. Returns `undefined` when there's
 *  nothing to persist — pass through to `insert_block_history`
 *  without a plan column (the Rust insert lets the column stay
 *  NULL). */
export function serializeExplainPlan(plan: unknown): string | undefined {
  if (plan === null || plan === undefined) return undefined;
  if (typeof plan === "string") {
    // Already serialized — typically the truncated-as-string
    // fallback path from `extract_plan_from_results`. Pass through
    // verbatim.
    return plan;
  }
  // Object / array / number / boolean — JSON.stringify so the column
  // round-trips. The consumer parses on read (frontend display layer
  // mount).
  try {
    return JSON.stringify(plan);
  } catch {
    // Defensive — circular references shouldn't reach this layer
    // (`DbResponse.plan` came from sqlx which can't produce them),
    // but if they do, drop the value rather than crash the insert.
    return undefined;
  }
}
