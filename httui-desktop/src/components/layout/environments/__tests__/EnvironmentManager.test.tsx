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
    // Always provide a list_env_variables fallback so loadVariables doesn't hang
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

    // 'dev' appears in sidebar (and possibly header); just confirm both names render
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

    // First env auto-selected on open — variables loaded once
    await waitFor(() => expect(varCalls).toBeGreaterThan(0));

    // Click the second env
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

  it("Escape key cancels the inline creator", async () => {
    const user = userEvent.setup();
    useEnvironmentStore.setState({ managerOpen: true });
    renderWithProviders(<EnvironmentManager />);

    await user.click(screen.getByText("+ New env"));
    expect(screen.getByTestId("env-mgr-new-env-name")).toBeInTheDocument();

    await user.keyboard("{Escape}");
    expect(screen.queryByTestId("env-mgr-new-env-name")).not.toBeInTheDocument();
  });
});
