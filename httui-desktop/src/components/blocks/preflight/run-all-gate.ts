import type { CheckResult } from "./preflight-types";

export interface RunAllGateInput {
  results: ReadonlyArray<CheckResult>;
  /** True when the user shift-clicked Run all — skips the gate. */
  overrideShift?: boolean;
}

export interface RunAllGateDecision {
  /** When true, the consumer must show the confirmation modal
   *  before running. */
  block: boolean;
  failedCount: number;
  skippedCount: number;
  /** Modal body when `block === true`; empty otherwise. */
  confirmCopy: string;
  /** Run-all report annotation; null when there's nothing to surface. */
  auditNote: string | null;
}

export function evaluatePreflightGate(
  input: RunAllGateInput,
): RunAllGateDecision {
  const failed = input.results.filter((r) => r.outcome === "fail").length;
  const skipped = input.results.filter((r) => r.outcome === "skip").length;
  const overrideShift = !!input.overrideShift;

  if (input.results.length === 0) {
    return {
      block: false,
      failedCount: 0,
      skippedCount: 0,
      confirmCopy: "",
      auditNote: null,
    };
  }

  if (failed === 0) {
    // Skipped items surface in the audit note for transparency.
    const auditNote =
      skipped > 0
        ? `${skipped} pre-flight check${skipped === 1 ? "" : "s"} skipped`
        : null;
    return {
      block: false,
      failedCount: 0,
      skippedCount: skipped,
      confirmCopy: "",
      auditNote,
    };
  }

  if (overrideShift) {
    return {
      block: false,
      failedCount: failed,
      skippedCount: skipped,
      confirmCopy: "",
      auditNote: `${failed} failed pre-flight, ran anyway via shift`,
    };
  }

  const word = failed === 1 ? "check" : "checks";
  return {
    block: true,
    failedCount: failed,
    skippedCount: skipped,
    confirmCopy: `${failed} pre-flight ${word} failed. Run anyway?`,
    auditNote: null,
  };
}
