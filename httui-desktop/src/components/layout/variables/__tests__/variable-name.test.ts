import { describe, expect, it } from "vitest";

import { validateVariableName } from "@/components/layout/variables/variable-name";

describe("validateVariableName", () => {
  it("accepts a non-empty UPPER_SNAKE name with no existing siblings", () => {
    expect(validateVariableName("API_BASE")).toEqual({ ok: true });
  });

  it("rejects empty input", () => {
    const r = validateVariableName("");
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.reason).toMatch(/required/i);
  });

  it("rejects whitespace-only input as empty", () => {
    const r = validateVariableName("   ");
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.reason).toMatch(/required/i);
  });

  it("rejects names containing internal whitespace", () => {
    const r = validateVariableName("API BASE");
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.reason).toMatch(/whitespace/i);
  });

  it("rejects names containing a dot (path separator collision)", () => {
    const r = validateVariableName("api.base");
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.reason).toMatch(/dot/i);
  });

  it("rejects exact duplicates against existing names", () => {
    const r = validateVariableName("API_BASE", ["DB_URL", "API_BASE"]);
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.reason).toMatch(/already exists/i);
  });

  it("rejects case-insensitive duplicates", () => {
    const r = validateVariableName("api_base", ["API_BASE"]);
    expect(r.ok).toBe(false);
  });

  it("trims whitespace before duplicate check", () => {
    const r = validateVariableName("  API  ", ["API"]);
    expect(r.ok).toBe(false);
  });

  it("ignores empty/whitespace siblings in the existing list", () => {
    expect(validateVariableName("API", ["", "  "])).toEqual({ ok: true });
  });
});
