import { describe, expect, it } from "vitest";

import { validateEnvName } from "@/components/layout/environments/env-name";

describe("validateEnvName", () => {
  it("accepts a plain alpha name with no existing siblings", () => {
    expect(validateEnvName("staging")).toEqual({ ok: true });
  });

  it("rejects empty input", () => {
    const r = validateEnvName("");
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.reason).toMatch(/required/i);
  });

  it("rejects whitespace-only input", () => {
    const r = validateEnvName("   ");
    expect(r.ok).toBe(false);
  });

  it("rejects names containing internal whitespace", () => {
    const r = validateEnvName("my staging");
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.reason).toMatch(/whitespace/i);
  });

  it("rejects names containing slashes", () => {
    expect(validateEnvName("dir/staging").ok).toBe(false);
    expect(validateEnvName("dir\\staging").ok).toBe(false);
  });

  it("rejects names starting with a dot", () => {
    const r = validateEnvName(".hidden");
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.reason).toMatch(/dot/i);
  });

  it("rejects names ending with .toml (case-insensitive)", () => {
    const r1 = validateEnvName("staging.toml");
    expect(r1.ok).toBe(false);
    if (!r1.ok) expect(r1.reason).toMatch(/automatically/i);
    expect(validateEnvName("staging.TOML").ok).toBe(false);
  });

  it("rejects exact duplicates against existing filenames (without suffix)", () => {
    const r = validateEnvName("staging", ["staging.toml"]);
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.reason).toMatch(/already exists/i);
  });

  it("rejects case-insensitive duplicates against existing names", () => {
    expect(validateEnvName("STAGING", ["staging.toml"]).ok).toBe(false);
  });

  it("treats `<name>.local.toml` as a collision with `<name>`", () => {
    const r = validateEnvName("staging", ["staging.local.toml"]);
    expect(r.ok).toBe(false);
  });

  it("does not collide with envs that share a prefix", () => {
    expect(validateEnvName("staging-2", ["staging.toml"])).toEqual({
      ok: true,
    });
  });
});
