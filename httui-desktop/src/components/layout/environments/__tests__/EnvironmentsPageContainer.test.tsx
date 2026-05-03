import { describe, it, expect, beforeEach, afterEach } from "vitest";

import {
  EnvironmentsPageContainer,
  envToSummary,
} from "@/components/layout/environments/EnvironmentsPageContainer";
import type { Environment, EnvVariable } from "@/lib/tauri/commands";
import { useEnvironmentStore } from "@/stores/environment";
import { clearTauriMocks, mockTauriCommand } from "@/test/mocks/tauri";
import { renderWithProviders, screen, waitFor } from "@/test/render";

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

describe("envToSummary", () => {
  it("derives the EnvironmentSummary fields from the backend Environment", () => {
    const summary = envToSummary(
      env({
        name: "staging",
        is_active: true,
        temporary: true,
        description: "preview",
        connections_used: ["payments-db", "users-db"],
      }),
      7,
    );
    expect(summary).toMatchObject({
      name: "staging",
      filename: "staging.toml",
      varCount: 7,
      connectionsUsedCount: 2,
      isActive: true,
      isPersonal: false,
      isTemporary: true,
      description: "preview",
    });
  });

  it("defaults connectionsUsedCount to 0 when no allowlist", () => {
    expect(envToSummary(env(), 0).connectionsUsedCount).toBe(0);
  });
});

beforeEach(() => {
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
    if (a.environmentId === "env-local") return [v({ id: "v1" })];
    if (a.environmentId === "env-prod") return [v({ id: "v2" }), v({ id: "v3" })];
    return [];
  });
});

afterEach(() => {
  clearTauriMocks();
  useEnvironmentStore.setState({
    environments: [],
    activeEnvironment: null,
    variablesVersion: 0,
  });
});

describe("EnvironmentsPageContainer", () => {
  it("renders the underlying EnvironmentsPage", async () => {
    renderWithProviders(<EnvironmentsPageContainer />);
    await waitFor(() => {
      expect(screen.getByTestId("environments-page")).toBeTruthy();
    });
  });

  it("renders one card per env with the loaded varCount", async () => {
    renderWithProviders(<EnvironmentsPageContainer />);
    await waitFor(() => {
      expect(screen.getByTestId("environment-card-local.toml")).toBeTruthy();
    });
    await waitFor(() => {
      expect(screen.getByTestId("environment-card-prod.toml")).toBeTruthy();
    });
    expect(
      screen.getByTestId("environment-card-local.toml-vars").textContent,
    ).toMatch(/1/);
    expect(
      screen.getByTestId("environment-card-prod.toml-vars").textContent,
    ).toMatch(/2/);
  });

  it("falls back to empty hint when listEnvironments returns nothing", async () => {
    mockTauriCommand("list_environments", () => []);
    renderWithProviders(<EnvironmentsPageContainer />);
    await waitFor(() => {
      expect(screen.getByTestId("environments-empty-hint")).toBeTruthy();
    });
  });
});
