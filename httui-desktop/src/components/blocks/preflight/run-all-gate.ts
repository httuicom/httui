// pure run-all gate over preflight `CheckResult[]`.
//
// The actual Run-all flow calls `evaluatePreflightGate`
// at the start, gets back a structured decision, and either:
//   - proceeds (block === false) — passes the `auditNote` (when
//     present) into the run-all report
//   - shows a confirmation modal (block === true) — `confirmCopy`
//     is the modal body; user can cancel or shift-click Run-all
//     to retry with `overrideShift = true`
//
// Pure logic; the modal + report integration carry to the consumer
// site (Run-all flow).

import type { CheckResult } from "./preflight-types";

export interface RunAllGateInput {
  results: ReadonlyArray<CheckResult>;
  /** True when the user shift-clicked Run all. Skips the gate even
   *  with failures present. */
  overrideShift?: boolean;
}

export interface RunAllGateDecision {
  /** When true, the consumer must show the confirmation modal
   *  before running. */
  block: boolean;
  /** Number of failed checks. Used by the modal copy + audit note. */
  failedCount: number;
  /** Number of skipped checks. Surfaced in the audit note when
   *  non-zero so users know what wasn't evaluated. */
  skippedCount: number;
  /** Modal body when `block === true`. Empty when `block === false`. */
  confirmCopy: string;
  /** Run-all report annotation when present (`null` when no
   *  preflight items, no failures, and no skipped items). */
  auditNote: string | null;
}

export function evaluatePreflightGate(
  input: RunAllGateInput,
): RunAllGateDecision {
  const failed = input.results.filter((r) => r.outcome === "fail").length;
  const skipped = input.results.filter((r) => r.outcome === "skip").length;
  const overrideShift = !!input.overrideShift;

  // No preflight items at all — clean run, no annotation.
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
    // All passes (or pass + skip mix) — proceed without modal.
    // Skipped items show up in the audit note for transparency.
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
    // User chose to bypass via shift. Audit note records the
    // override per the spec's "ran anyway via shift" requirement.
    return {
      block: false,
      failedCount: failed,
      skippedCount: skipped,
      confirmCopy: "",
      auditNote: `${failed} failed pre-flight, ran anyway via shift`,
    };
  }

  // Block + propose the modal copy.
  const word = failed === 1 ? "check" : "checks";
  return {
    block: true,
    failedCount: failed,
    skippedCount: skipped,
    confirmCopy: `${failed} pre-flight ${word} failed. Run anyway?`,
    auditNote: null,
  };
}
