import { describe, expect, it } from "vitest";

import {
  pillGlyph,
  pillKindFromResult,
  type CheckResult,
} from "../preflight-types";

describe("pillKindFromResult", () => {
  it("maps each outcome 1:1 to a pill kind", () => {
    expect(pillKindFromResult({ outcome: "pass" } as CheckResult)).toBe("pass");
    expect(
      pillKindFromResult({ outcome: "fail", reason: "x" } as CheckResult),
    ).toBe("fail");
    expect(
      pillKindFromResult({ outcome: "skip", reason: "x" } as CheckResult),
    ).toBe("skip");
  });
});

describe("pillGlyph", () => {
  it("matches the canvas spec glyphs", () => {
    expect(pillGlyph("pass")).toBe("✓");
    expect(pillGlyph("fail")).toBe("✗");
    expect(pillGlyph("skip")).toBe("–");
    expect(pillGlyph("running")).toBe("◌");
  });
});
