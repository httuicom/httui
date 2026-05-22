import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { renderWithProviders, screen, waitFor } from "@/test/render";
import userEvent from "@testing-library/user-event";
import { EnvironmentManager } from "@/components/layout/environments/EnvironmentManager";
import { useEnvironmentStore } from "@/stores/environment";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";
import type { Environment, EnvVariable } from "@/lib/tauri/commands";

const mkEnv = (id: string, name: string, isActive = false): Environment => ({
  id,
  name,
  is_active: isActive,
  created_at: "2026-01-01T00:00:00Z",
});

const mkVar = (
  id: string,
  envId: string,
  key: string,
  value: string,
  isSecret = false,
): EnvVariable => ({
  id,
  environment_id: envId,
  key,
  value,
  is_secret: isSecret,
  created_at: "2026-01-01T00:00:00Z",
});

function resetStore() {
  useEnvironmentStore.setState({
    environments: [],
    activeEnvironment: null,
    managerOpen: false,
    variablesVersion: 0,
  });
}

describe("EnvironmentManager", () => {
  beforeEach(() => {
    resetStore();
    clearTauriMocks();
    mockTauriCommand("list_env_variables", () => []);
  });

  afterEach(() => {
    clearTauriMocks();
  });

  it("renders nothing when managerOpen is false", () => {
    const { container } = renderWithProviders(<EnvironmentManager />);
    expect(container.querySelector("[role]")).toBeNull();
    expect(screen.queryByText("Environments")).not.toBeInTheDocument();
  });

  it("renders header and Close button when open", () => {
    useEnvironmentStore.setState({ managerOpen: true });
    renderWithProviders(<EnvironmentManager />);

    expect(screen.getByText("Environments")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /close/i })).toBeInTheDocument();
  });

  it("Close button triggers store closeManager", async () => {
    const user = userEvent.setup();
    useEnvironmentStore.setState({ managerOpen: true });
    renderWithProviders(<EnvironmentManager />);

    await user.click(screen.getByRole("button", { name: /close/i }));
    expect(useEnvironmentStore.getState().managerOpen).toBe(false);
  });

  it("lists existing environments and shows 'active' badge for active one", () => {
    useEnvironmentStore.setState({
      managerOpen: true,
      environments: [mkEnv("a", "dev"), mkEnv("b", "prod", true)],
      activeEnvironment: mkEnv("b", "prod", true),
    });
    renderWithProviders(<EnvironmentManager />);

    expect(screen.getAllByText("dev").length).toBeGreaterThan(0);
    expect(screen.getAllByText("prod").length).toBeGreaterThan(0);
    expect(screen.getByText("active")).toBeInTheDocument();
  });

  it("renders 'Create an environment' empty state when list is empty", () => {
    useEnvironmentStore.setState({ managerOpen: true });
    renderWithProviders(<EnvironmentManager />);

    expect(
      screen.getByText("Create an environment to get started"),
    ).toBeInTheDocument();
  });

  it("clicking an environment selects it and loads its variables", async () => {
    const user = userEvent.setup();
    let varCalls = 0;
    mockTauriCommand("list_env_variables", () => {
      varCalls++;
      return [mkVar("v1", "a", "TOKEN", "abc")];
    });

    useEnvironmentStore.setState({
      managerOpen: true,
      environments: [mkEnv("a", "dev"), mkEnv("b", "prod")],
    });
    renderWithProviders(<EnvironmentManager />);

    await waitFor(() => expect(varCalls).toBeGreaterThan(0));

    await user.click(screen.getByText("prod"));
    await waitFor(() => expect(varCalls).toBeGreaterThan(1));
  });

  it("'New env' creator: enter name + Enter → IPC create called", async () => {
    const user = userEvent.setup();
    let createdName: unknown = "";
    mockTauriCommand("create_environment", (args) => {
      createdName = (args as { name: string }).name;
    });
    mockTauriCommand("list_environments", () => [mkEnv("new", "qa")]);

    useEnvironmentStore.setState({ managerOpen: true });
    renderWithProviders(<EnvironmentManager />);

    await user.click(screen.getByText("+ New env"));
    const input = screen.getByTestId("env-mgr-new-env-name");
    await user.type(input, "qa{Enter}");

    await waitFor(() => expect(createdName).toBe("qa"));
  });

  it("'New env' creator does nothing on empty name + Enter", async () => {
    const user = userEvent.setup();
    let called = false;
    mockTauriCommand("create_environment", () => {
      called = true;
    });

    useEnvironmentStore.setState({ managerOpen: true });
    renderWithProviders(<EnvironmentManager />);

    await user.click(screen.getByText("+ New env"));
    await user.type(screen.getByTestId("env-mgr-new-env-name"), "{Enter}");

    expect(called).toBe(false);
  });

  it("renders a VariableValueRow per loaded variable of the selected env", async () => {
    mockTauriCommand("list_env_variables", () => [
      mkVar("v1", "a", "API_BASE", "http://localhost"),
      mkVar("v2", "a", "TOKEN", "", true),
    ]);
    useEnvironmentStore.setState({
      managerOpen: true,
      environments: [mkEnv("a", "dev")],
    });
    renderWithProviders(<EnvironmentManager />);
    await waitFor(() => {
      expect(
        document.querySelectorAll('[data-testid="variable-value-row-dev"]')
          .length,
      ).toBe(2);
    });
  });

  it("'+ New variable' opens the inline form and persists via setVariable", async () => {
    let setCalled: { key?: string } | null = null;
    mockTauriCommand("list_env_variables", () => []);
    mockTauriCommand("set_env_variable", (args) => {
      setCalled = args as { key: string };
      return mkVar("v-new", "a", "FRESH", "value");
    });
    useEnvironmentStore.setState({
      managerOpen: true,
      environments: [mkEnv("a", "dev")],
    });
    const user = userEvent.setup();
    renderWithProviders(<EnvironmentManager />);
    await user.click(await screen.findByText("+ New variable"));
    await user.type(await screen.findByTestId("new-variable-name"), "FRESH");
    await user.type(screen.getByTestId("new-variable-value"), "value");
    await user.click(screen.getByTestId("new-variable-save"));
    await waitFor(() => {
      expect(setCalled).not.toBeNull();
    });
    expect(setCalled).toMatchObject({ key: "FRESH" });
  });

  it("'Set active' on the selected env dispatches set_active_environment", async () => {
    let activeArgs: { id?: string | null } | null = null;
    mockTauriCommand("set_active_environment", (args) => {
      activeArgs = args as { id: string | null };
      return undefined;
    });
    useEnvironmentStore.setState({
      managerOpen: true,
      environments: [mkEnv("a", "dev")],
    });
    renderWithProviders(<EnvironmentManager />);
    const user = userEvent.setup();
    await user.click(await screen.findByText("Set active"));
    await waitFor(() => {
      expect(activeArgs).not.toBeNull();
    });
    expect(activeArgs).toEqual({ id: "a" });
  });

  it("Duplicate IconButton dispatches duplicate_environment", async () => {
    let dupCalled: { sourceId?: string; newName?: string } | null = null;
    mockTauriCommand("duplicate_environment", (args) => {
      dupCalled = args as { sourceId: string; newName: string };
      return mkEnv("a-copy", "dev-copy");
    });
    useEnvironmentStore.setState({
      managerOpen: true,
      environments: [mkEnv("a", "dev")],
    });
    renderWithProviders(<EnvironmentManager />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: /duplicate/i }));
    await waitFor(() => {
      expect(dupCalled).not.toBeNull();
    });
    expect(dupCalled).toMatchObject({ sourceId: "a", newName: "dev-copy" });
  });

  it("per-var X icon dispatches delete_env_variable", async () => {
    let deletedId: { id?: string } | null = null;
    mockTauriCommand("list_env_variables", () => [
      mkVar("v1", "a", "API_BASE", "x"),
    ]);
    mockTauriCommand("delete_env_variable", (args) => {
      deletedId = args as { id: string };
      return undefined;
    });
    useEnvironmentStore.setState({
      managerOpen: true,
      environments: [mkEnv("a", "dev")],
    });
    renderWithProviders(<EnvironmentManager />);
    const user = userEvent.setup();
    await user.click(
      await screen.findByTestId("variable-value-row-dev-delete"),
    );
    await waitFor(() => {
      expect(deletedId).not.toBeNull();
    });
    expect(deletedId).toMatchObject({ id: "v1" });
  });

  it("Escape key cancels the inline creator", async () => {
    const user = userEvent.setup();
    useEnvironmentStore.setState({ managerOpen: true });
    renderWithProviders(<EnvironmentManager />);

    await user.click(screen.getByText("+ New env"));
    expect(screen.getByTestId("env-mgr-new-env-name")).toBeInTheDocument();

    await user.keyboard("{Escape}");
    expect(
      screen.queryByTestId("env-mgr-new-env-name"),
    ).not.toBeInTheDocument();
  });
});
