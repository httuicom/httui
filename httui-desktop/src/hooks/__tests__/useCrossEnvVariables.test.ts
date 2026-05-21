import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";

import { useCrossEnvVariables } from "@/hooks/useCrossEnvVariables";
import type { Environment, EnvVariable } from "@/lib/tauri/commands";
import { useEnvironmentStore } from "@/stores/environment";
import { clearTauriMocks, mockTauriCommand } from "@/test/mocks/tauri";

const env = (over: Partial<Environment> = {}): Environment => ({
  id: "env-local",
  name: "local",
  is_active: false,
  created_at: "",
  description: null,
  ...over,
});

const v = (over: Partial<EnvVariable> = {}): EnvVariable => ({
  id: "v1",
  environment_id: "env-local",
  key: "API_BASE",
  value: "x",
  is_secret: false,
  created_at: "",
  ...over,
});

beforeEach(() => {
  useEnvironmentStore.setState({
    environments: [],
    activeEnvironment: null,
    variablesVersion: 0,
  });
  clearTauriMocks();
});

afterEach(() => {
  clearTauriMocks();
  useEnvironmentStore.setState({
    environments: [],
    activeEnvironment: null,
    variablesVersion: 0,
  });
  vi.clearAllMocks();
});

describe("useCrossEnvVariables", () => {
  it("returns an empty list when there are no environments", () => {
    const { result } = renderHook(() => useCrossEnvVariables());
    expect(result.current).toEqual([]);
  });

  it("fans out list_env_variables across every environment", async () => {
    useEnvironmentStore.setState({
      environments: [
        env({ id: "env-local", name: "local" }),
        env({ id: "env-prod", name: "prod" }),
      ],
    });
    mockTauriCommand("list_env_variables", (args) => {
      const a = args as { environmentId?: string };
      if (a.environmentId === "env-local") {
        return [v({ id: "v1", key: "API_BASE", value: "http://local" })];
      }
      if (a.environmentId === "env-prod") {
        return [
          v({
            id: "v2",
            environment_id: "env-prod",
            key: "API_BASE",
            value: "https://prod",
          }),
        ];
      }
      return [];
    });

    const { result } = renderHook(() => useCrossEnvVariables());

    await waitFor(() => expect(result.current).toHaveLength(2));
    const byName = Object.fromEntries(
      result.current.map((b) => [b.env.name, b]),
    );
    expect(byName.local.vars[0].value).toBe("http://local");
    expect(byName.prod.vars[0].value).toBe("https://prod");
  });

  it("swallows a per-env failure and yields [] for that env", async () => {
    useEnvironmentStore.setState({
      environments: [
        env({ id: "env-local", name: "local" }),
        env({ id: "env-prod", name: "prod" }),
      ],
    });
    mockTauriCommand("list_env_variables", (args) => {
      const a = args as { environmentId?: string };
      if (a.environmentId === "env-prod") {
        throw new Error("prod read failed");
      }
      return [v({ id: "v1", key: "API_BASE" })];
    });

    const { result } = renderHook(() => useCrossEnvVariables());

    await waitFor(() => expect(result.current).toHaveLength(2));
    const byName = Object.fromEntries(
      result.current.map((b) => [b.env.name, b]),
    );
    expect(byName.local.vars).toHaveLength(1);
    expect(byName.prod.vars).toEqual([]);
  });

  it("re-runs the fan-out when variablesVersion bumps", async () => {
    useEnvironmentStore.setState({
      environments: [env({ id: "env-local", name: "local" })],
    });
    let calls = 0;
    mockTauriCommand("list_env_variables", () => {
      calls += 1;
      return [v({ id: `v${calls}`, value: `value-${calls}` })];
    });

    const { result } = renderHook(() => useCrossEnvVariables());
    await waitFor(() =>
      expect(result.current[0]?.vars[0]?.value).toBe("value-1"),
    );

    act(() => {
      useEnvironmentStore.setState({ variablesVersion: 1 });
    });

    await waitFor(() =>
      expect(result.current[0]?.vars[0]?.value).toBe("value-2"),
    );
    expect(calls).toBe(2);
  });

  it("ignores a fan-out that resolves after unmount (cancelled guard)", async () => {
    useEnvironmentStore.setState({
      environments: [env({ id: "env-local", name: "local" })],
    });
    let release!: (vars: EnvVariable[]) => void;
    const pending = new Promise<EnvVariable[]>((resolve) => {
      release = resolve;
    });
    mockTauriCommand("list_env_variables", () => pending);

    const { result, unmount } = renderHook(() => useCrossEnvVariables());
    expect(result.current).toEqual([]);

    unmount();
    // Resolve after the cleanup ran — the cancelled guard must drop it.
    await act(async () => {
      release([v({ id: "late" })]);
      await pending;
    });

    expect(result.current).toEqual([]);
  });
});
