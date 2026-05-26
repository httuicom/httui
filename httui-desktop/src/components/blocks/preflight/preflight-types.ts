// TS shape mirror of `httui-core::preflight::CheckResult`.
//
// Rust serde tags `CheckResult` on `outcome` (snake_case). The
// frontend just types the shape so the React renderer compiles
// without round-tripping through Rust-emitted types. Rust stays
// canonical for the evaluator.

export type CheckResult =
  | { outcome: "pass" }
  | { outcome: "fail"; reason: string }
  | { outcome: "skip"; reason: string };

/** Pill kind — used by the renderer to pick icon + color. The
 *  "running" state isn't an evaluator outcome (the evaluator is
 *  synchronous); it represents the consumer-side "Re-check is in
 *  flight" state and is a separate prop on the row component. */
export type PillKind = "pass" | "fail" | "skip" | "running";

export function pillKindFromResult(result: CheckResult): PillKind {
  return result.outcome;
}

/** Glyph used by the pill renderer: ✓ (pass), ✗ (fail), – (skip),
 * ◌ (running). Matches the spec. */
export function pillGlyph(kind: PillKind): string {
  switch (kind) {
    case "pass":
      return "✓";
    case "fail":
      return "✗";
    case "skip":
      return "–";
    case "running":
      return "◌";
  }
}
