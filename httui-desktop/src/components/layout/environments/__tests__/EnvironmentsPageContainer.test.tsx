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

  it("Clone menu opens the inline form and dispatches duplicate_environment", async () => {
    let captured: { sourceId?: string; newName?: string } | null = null;
    mockTauriCommand("duplicate_environment", (args) => {
      captured = args as { sourceId: string; newName: string };
      return env({ id: "env-prod-copy", name: "prod-copy" });
    });

    const { default: userEvent } = await import("@testing-library/user-event");
    renderWithProviders(<EnvironmentsPageContainer />);
    const user = userEvent.setup();

    const moreBtn = await screen.findByTestId(
      "environment-card-prod.toml-more",
    );
    await user.click(moreBtn);
    await user.click(await screen.findByText("Clone"));
    await user.type(
      await screen.findByTestId("clone-environment-name"),
      "prod-copy",
    );
    await user.click(screen.getByTestId("clone-environment-save"));

    await waitFor(() => {
      expect(captured).not.toBeNull();
    });
    expect(captured).toMatchObject({
      sourceId: "env-prod",
      newName: "prod-copy",
    });
  });

  it("activates an env on card click and dispatches set_active_environment", async () => {
    let setActiveCalled: { id?: string } | null = null;
    mockTauriCommand("set_active_environment", (args) => {
      setActiveCalled = args as { id: string };
      return undefined;
    });
    const { default: userEvent } = await import(
      "@testing-library/user-event"
    );
    renderWithProviders(<EnvironmentsPageContainer />);
    const card = await screen.findByTestId("environment-card-prod.toml");
    const activateBtn = card.querySelector("button");
    await userEvent.setup().click(activateBtn!);
    await waitFor(() => {
      expect(setActiveCalled).not.toBeNull();
    });
    expect(setActiveCalled).toEqual({ id: "env-prod" });
  });

  it("Rename menu opens the form and dispatches rename_environment", async () => {
    let captured: { oldId?: string; newName?: string } | null = null;
    mockTauriCommand("rename_environment", (args) => {
      captured = args as { oldId: string; newName: string };
      return env({ id: "env-prod-renamed", name: "prod-renamed" });
    });
    const { default: userEvent } = await import("@testing-library/user-event");
    renderWithProviders(<EnvironmentsPageContainer />);
    const user = userEvent.setup();

    await user.click(
      await screen.findByTestId("environment-card-prod.toml-more"),
    );
    await user.click(await screen.findByText("Rename"));
    const input = (await screen.findByTestId(
      "rename-environment-name",
    )) as HTMLInputElement;
    await user.clear(input);
    await user.type(input, "prod-renamed");
    await user.click(screen.getByTestId("rename-environment-save"));

    await waitFor(() => {
      expect(captured).not.toBeNull();
    });
    expect(captured).toMatchObject({
      oldId: "env-prod",
      newName: "prod-renamed",
    });
  });

  it("Delete menu type-to-confirm dispatches delete_environment", async () => {
    let deleteCalled: { id?: string } | null = null;
    mockTauriCommand("delete_environment", (args) => {
      deleteCalled = args as { id: string };
      return undefined;
    });
    const { default: userEvent } = await import("@testing-library/user-event");
    renderWithProviders(<EnvironmentsPageContainer />);
    const user = userEvent.setup();

    await user.click(
      await screen.findByTestId("environment-card-prod.toml-more"),
    );
    await user.click(await screen.findByText("Delete"));
    await user.type(
      await screen.findByTestId("delete-environment-confirm-input"),
      "prod",
    );
    await user.click(
      screen.getByTestId("delete-environment-confirm-submit"),
    );

    await waitFor(() => {
      expect(deleteCalled).not.toBeNull();
    });
    expect(deleteCalled).toEqual({ id: "env-prod" });
  });

  it("Clone form Cancel closes the inline slot without dispatching", async () => {
    let dupeCalls = 0;
    mockTauriCommand("duplicate_environment", () => {
      dupeCalls += 1;
      return env({ id: "x", name: "x" });
    });
    const { default: userEvent } = await import("@testing-library/user-event");
    renderWithProviders(<EnvironmentsPageContainer />);
    const user = userEvent.setup();
    await user.click(
      await screen.findByTestId("environment-card-prod.toml-more"),
    );
    await user.click(await screen.findByText("Clone"));
    expect(screen.getByTestId("clone-environment-form")).toBeTruthy();
    await user.click(screen.getByTestId("clone-environment-cancel"));
    await waitFor(() => {
      expect(screen.queryByTestId("clone-environment-form")).toBeNull();
    });
    expect(dupeCalls).toBe(0);
  });
});
