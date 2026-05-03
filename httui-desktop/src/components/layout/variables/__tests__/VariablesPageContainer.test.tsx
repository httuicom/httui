import { describe, it, expect, beforeEach, afterEach } from "vitest";

import { VariablesPageContainer } from "@/components/layout/variables/VariablesPageContainer";
import { mergeCrossEnvVariables } from "@/components/layout/variables/VariablesPageContainer";
import type { Environment, EnvVariable } from "@/lib/tauri/commands";
import { useWorkspaceStore } from "@/stores/workspace";
import { useEnvironmentStore } from "@/stores/environment";
import { clearTauriMocks, mockTauriCommand } from "@/test/mocks/tauri";
import { renderWithProviders, screen, waitFor } from "@/test/render";

const env = (over: Partial<Environment> = {}): Environment => ({
  id: "env-local",
  name: "local",
  description: null,
  is_active: false,
  created_at: "",
  updated_at: "",
  ...over,
});

const v = (over: Partial<EnvVariable> = {}): EnvVariable => ({
  id: "var-1",
  environment_id: "env-local",
  key: "API_BASE",
  value: "http://localhost",
  is_secret: false,
  created_at: "",
  ...over,
});

describe("mergeCrossEnvVariables", () => {
  it("merges per-env variable lists into one row per key", () => {
    const local = env({ id: "env-local", name: "local" });
    const prod = env({ id: "env-prod", name: "prod" });
    const rows = mergeCrossEnvVariables([
      {
        env: local,
        vars: [
          v({ id: "v1", environment_id: local.id, key: "API_BASE", value: "x" }),
          v({ id: "v2", environment_id: local.id, key: "DB_URL", value: "y" }),
        ],
      },
      {
        env: prod,
        vars: [
          v({ id: "v3", environment_id: prod.id, key: "API_BASE", value: "z" }),
        ],
      },
    ]);

    const byKey = Object.fromEntries(rows.map((r) => [r.key, r]));
    expect(rows).toHaveLength(2);
    expect(byKey.API_BASE.values).toEqual({ local: "x", prod: "z" });
    expect(byKey.DB_URL.values).toEqual({ local: "y" });
    expect(byKey.API_BASE.scope).toBe("workspace");
    expect(byKey.API_BASE.isSecret).toBe(false);
  });

  it("flips isSecret when any env marks the var as secret", () => {
    const local = env({ id: "env-local", name: "local" });
    const prod = env({ id: "env-prod", name: "prod" });
    const rows = mergeCrossEnvVariables([
      {
        env: local,
        vars: [v({ key: "TOKEN", value: "plain", is_secret: false })],
      },
      {
        env: prod,
        vars: [
          v({ key: "TOKEN", environment_id: prod.id, value: "", is_secret: true }),
        ],
      },
    ]);
    expect(rows[0].isSecret).toBe(true);
  });

  it("returns an empty list for an empty input", () => {
    expect(mergeCrossEnvVariables([])).toEqual([]);
  });

  it("seeds isSecret from the first occurrence of the key", () => {
    const local = env({ id: "env-local", name: "local" });
    const rows = mergeCrossEnvVariables([
      { env: local, vars: [v({ is_secret: true })] },
    ]);
    expect(rows[0].isSecret).toBe(true);
  });
});

beforeEach(() => {
  useWorkspaceStore.setState({ vaultPath: "/tmp/vault" });
  useEnvironmentStore.setState({
    environments: [],
    activeEnvironment: null,
    variablesVersion: 0,
  });
  clearTauriMocks();
  mockTauriCommand("list_environments", () => [
    env({ id: "env-local", name: "local", is_active: true }),
    env({ id: "env-prod", name: "prod" }),
  ]);
  mockTauriCommand("list_env_variables", (args) => {
    const a = args as { environmentId?: string };
    if (a.environmentId === "env-local") {
      return [v({ id: "v1", key: "API_BASE", value: "http://localhost" })];
    }
    if (a.environmentId === "env-prod") {
      return [
        v({
          id: "v2",
          environment_id: "env-prod",
          key: "API_BASE",
          value: "https://api.example",
        }),
      ];
    }
    return [];
  });
  mockTauriCommand("grep_var_uses", () => [
    { file_path: "notes/x.md", line: 12, snippet: "{{API_BASE}}" },
  ]);
});

afterEach(() => {
  clearTauriMocks();
  useWorkspaceStore.setState({ vaultPath: null });
  useEnvironmentStore.setState({
    environments: [],
    activeEnvironment: null,
    variablesVersion: 0,
  });
});

describe("VariablesPageContainer", () => {
  it("renders the underlying VariablesPage", async () => {
    renderWithProviders(<VariablesPageContainer />);
    await waitFor(() => {
      expect(screen.getByTestId("variables-page")).toBeTruthy();
    });
  });

  it("merges per-env variables into rows visible to the page", async () => {
    renderWithProviders(<VariablesPageContainer />);
    await waitFor(() => {
      expect(screen.getByTestId("variables-row-API_BASE")).toBeTruthy();
    });
    await waitFor(() => {
      const cell = screen.getByTestId("variables-row-API_BASE-value-local");
      expect(cell.textContent).toBe("http://localhost");
    });
  });

  it("annotates rows with the vault-grep usesCount", async () => {
    renderWithProviders(<VariablesPageContainer />);
    await waitFor(() => {
      const usesCell = screen.getByTestId("variables-row-API_BASE-uses");
      expect(usesCell.textContent).toBe("1");
    });
  });

  it("survives an empty environments list (renders empty page)", async () => {
    mockTauriCommand("list_environments", () => []);
    renderWithProviders(<VariablesPageContainer />);
    await waitFor(() => {
      expect(screen.getByTestId("variables-page")).toBeTruthy();
    });
    expect(screen.queryByTestId("variables-row-API_BASE")).toBeNull();
  });

  it("renders even when grep_var_uses fails (counts default to 0)", async () => {
    mockTauriCommand("grep_var_uses", () => {
      throw new Error("vault-grep failed");
    });
    renderWithProviders(<VariablesPageContainer />);
    await waitFor(() => {
      expect(screen.getByTestId("variables-row-API_BASE")).toBeTruthy();
    });
    const usesCell = screen.getByTestId("variables-row-API_BASE-uses");
    expect(usesCell.textContent).toBe("—");
  });
});
