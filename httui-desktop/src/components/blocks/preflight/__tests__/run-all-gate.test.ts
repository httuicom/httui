import { describe, expect, it } from "vitest";

import type { CheckResult } from "../preflight-types";
import { evaluatePreflightGate } from "../run-all-gate";

const PASS: CheckResult = { outcome: "pass" };
const fail = (reason: string): CheckResult => ({ outcome: "fail", reason });
const skip = (reason: string): CheckResult => ({ outcome: "skip", reason });

describe("evaluatePreflightGate", () => {
  it("does not block when there are no preflight items", () => {
    const d = evaluatePreflightGate({ results: [] });
    expect(d.block).toBe(false);
    expect(d.failedCount).toBe(0);
    expect(d.auditNote).toBeNull();
  });

  it("does not block when all checks pass", () => {
    const d = evaluatePreflightGate({ results: [PASS, PASS] });
    expect(d.block).toBe(false);
    expect(d.failedCount).toBe(0);
    expect(d.auditNote).toBeNull();
  });

  it("does not block on pass + skip mix; surfaces skipped count in audit", () => {
    const d = evaluatePreflightGate({
      results: [PASS, skip("vault not a git repo")],
    });
    expect(d.block).toBe(false);
    expect(d.skippedCount).toBe(1);
    expect(d.auditNote).toBe("1 pre-flight check skipped");
  });

  it("agrees plural in the skipped audit note", () => {
    const d = evaluatePreflightGate({
      results: [skip("a"), skip("b"), skip("c")],
    });
    expect(d.auditNote).toBe("3 pre-flight checks skipped");
  });

  it("blocks when any check fails (no shift override)", () => {
    const d = evaluatePreflightGate({
      results: [PASS, fail("missing connection")],
    });
    expect(d.block).toBe(true);
    expect(d.failedCount).toBe(1);
    expect(d.confirmCopy).toBe("1 pre-flight check failed. Run anyway?");
    expect(d.auditNote).toBeNull();
  });

  it("agrees plural in the confirm copy", () => {
    const d = evaluatePreflightGate({
      results: [fail("a"), fail("b"), fail("c")],
    });
    expect(d.confirmCopy).toBe("3 pre-flight checks failed. Run anyway?");
  });

  it("does not block when shift-override is set even with failures", () => {
    const d = evaluatePreflightGate({
      results: [fail("a"), fail("b")],
      overrideShift: true,
    });
    expect(d.block).toBe(false);
    expect(d.failedCount).toBe(2);
    expect(d.auditNote).toBe("2 failed pre-flight, ran anyway via shift");
  });

  it("agrees plural in the override audit note", () => {
    const d = evaluatePreflightGate({
      results: [fail("a")],
      overrideShift: true,
    });
    expect(d.auditNote).toBe("1 failed pre-flight, ran anyway via shift");
  });

  it("counts only fails toward the failed count (skip + pass don't)", () => {
    const d = evaluatePreflightGate({
      results: [PASS, skip("a"), fail("b"), fail("c")],
    });
    expect(d.failedCount).toBe(2);
    expect(d.skippedCount).toBe(1);
  });

  it("clears confirmCopy when not blocking", () => {
    const passing = evaluatePreflightGate({ results: [PASS] });
    expect(passing.confirmCopy).toBe("");
    const overridden = evaluatePreflightGate({
      results: [fail("a")],
      overrideShift: true,
    });
    expect(overridden.confirmCopy).toBe("");
  });
});
