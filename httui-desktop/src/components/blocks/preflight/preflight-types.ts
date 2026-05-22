// TS mirror of `httui-core::preflight::CheckResult` (tagged on `outcome`).

export type CheckResult =
  | { outcome: "pass" }
  | { outcome: "fail"; reason: string }
  | { outcome: "skip"; reason: string };

/** Pill kind. "running" is not an evaluator outcome — it's the consumer-side "re-check in flight" state. */
export type PillKind = "pass" | "fail" | "skip" | "running";

export function pillKindFromResult(result: CheckResult): PillKind {
  return result.outcome;
}

/** Glyph for each pill kind: ✓ pass · ✗ fail · – skip · ◌ running. */
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
