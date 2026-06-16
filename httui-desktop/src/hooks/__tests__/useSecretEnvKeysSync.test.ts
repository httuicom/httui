import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook } from "@testing-library/react";

const bundlesMock = vi.fn();
vi.mock("@/hooks/useCrossEnvVariables", () => ({
  useCrossEnvVariables: () => bundlesMock(),
}));

import { useSecretEnvKeysSync } from "../useSecretEnvKeysSync";
import { useEnvironmentStore } from "@/stores/environment";
import { isSecretEnvKey, setSecretEnvKeys } from "@/lib/blocks/secret-env-keys";

const env = { id: "e1", name: "local", is_active: true } as never;

beforeEach(() => {
  setSecretEnvKeys([]);
  useEnvironmentStore.setState({ activeEnvironment: env });
});
afterEach(() => {
  useEnvironmentStore.setState({ activeEnvironment: null });
  vi.clearAllMocks();
});

describe("useSecretEnvKeysSync", () => {
  it("marks the active env's is_secret vars as secret keys", () => {
    bundlesMock.mockReturnValue([
      {
        env,
        vars: [
          { key: "TOKEN", value: "", is_secret: true },
          { key: "BASE_URL", value: "x", is_secret: false },
        ],
      },
    ]);
    renderHook(() => useSecretEnvKeysSync());
    expect(isSecretEnvKey("TOKEN")).toBe(true);
    expect(isSecretEnvKey("BASE_URL")).toBe(false);
  });

  it("clears the set when there is no active environment", () => {
    setSecretEnvKeys(["TOKEN"]);
    useEnvironmentStore.setState({ activeEnvironment: null });
    bundlesMock.mockReturnValue([]);
    renderHook(() => useSecretEnvKeysSync());
    expect(isSecretEnvKey("TOKEN")).toBe(false);
  });
});
