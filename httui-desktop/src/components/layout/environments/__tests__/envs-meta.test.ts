import { describe, expect, it } from "vitest";

import {
  envNameFromFilename,
  isPersonalEnvFilename,
  sortEnvironments,
  type EnvironmentSummary,
} from "@/components/layout/environments/envs-meta";

function env(over: Partial<EnvironmentSummary>): EnvironmentSummary {
  return {
    name: "local",
    filename: "local.toml",
    varCount: 0,
    connectionsUsedCount: 0,
    isActive: false,
    isPersonal: false,
    isTemporary: false,
    ...over,
  };
}

describe("isPersonalEnvFilename", () => {
  it("returns true for `<name>.local.toml`", () => {
    expect(isPersonalEnvFilename("local.local.toml")).toBe(true);
    expect(isPersonalEnvFilename("staging.local.toml")).toBe(true);
  });

  it("returns false for plain `<name>.toml`", () => {
    expect(isPersonalEnvFilename("local.toml")).toBe(false);
    expect(isPersonalEnvFilename("staging.toml")).toBe(false);
  });

  it("returns false for unrelated names", () => {
    expect(isPersonalEnvFilename("local")).toBe(false);
    expect(isPersonalEnvFilename(".local.toml.bak")).toBe(false);
  });
});

describe("envNameFromFilename", () => {
  it("strips `.local.toml`", () => {
    expect(envNameFromFilename("staging.local.toml")).toBe("staging");
  });

  it("strips `.toml`", () => {
    expect(envNameFromFilename("local.toml")).toBe("local");
  });

  it("returns the input unchanged when it has no known suffix", () => {
    expect(envNameFromFilename("README")).toBe("README");
  });
});

describe("sortEnvironments", () => {
  it("sorts alphabetically (case-insensitive) regardless of isActive", () => {
    // cards stay anchored so the FLIP swap of the ACTIVE pill
    // across positions actually reads as motion.
    const out = sortEnvironments([
      env({ name: "zeta", filename: "zeta.toml" }),
      env({ name: "Alpha", filename: "alpha.toml" }),
      env({ name: "beta", filename: "beta.toml", isActive: true }),
    ]);
    expect(out.map((e) => e.name)).toEqual(["Alpha", "beta", "zeta"]);
  });

  it("does not mutate the input", () => {
    const input = [env({ name: "z" }), env({ name: "a" })];
    const out = sortEnvironments(input);
    expect(input.map((e) => e.name)).toEqual(["z", "a"]);
    expect(out.map((e) => e.name)).toEqual(["a", "z"]);
  });

  it("falls back to alpha when nothing is active", () => {
    const out = sortEnvironments([
      env({ name: "zeta" }),
      env({ name: "alpha" }),
    ]);
    expect(out.map((e) => e.name)).toEqual(["alpha", "zeta"]);
  });
});
