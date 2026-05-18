// frontend type mirror of `httui-core::explain::PlanNode`.
//
// The Rust side is canonical (parser lives there). This file is the
// TS shape the React renderer consumes — keeps backend canonical
// while letting the UI compile without round-tripping through any
// Rust-emitted types.
//
// Field meanings: see `httui-core/src/explain/node.rs` and the
// canvas Workbench §SqlBlock mock.

export interface PlanNode {
  /** Operation kind — "Limit", "Sort", "Hash Anti Join", "Seq Scan", … */
  op: string;
  /** Free-text target; e.g. `r.created_at DESC`, `routes`, `(rows=50)`. */
  target: string;
  /** Cost range as the driver reports it ("0.42..18.7"). */
  cost: string;
  rows: number;
  /** Share of total query cost, 0..100. */
  pct: number;
  /** Heuristic warn flag — Seq Scan over 10k rows, hash anti-join with
   *  >50% cost share, Sort exceeding work_mem, etc. */
  warn: boolean;
  children: ReadonlyArray<PlanNode>;
}

/**
 * Format `rows` with the user's locale separators. The canvas mock
 * shows comma-grouped large numbers ("1,234,567 rows").
 */
export function formatRows(rows: number, locale?: string): string {
  return rows.toLocaleString(locale);
}
