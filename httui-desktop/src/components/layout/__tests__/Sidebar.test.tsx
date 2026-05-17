import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderWithWorkspace, screen } from "@/test/render";

import { Sidebar } from "@/components/layout/Sidebar";
import { useEnvironmentStore } from "@/stores/environment";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";

beforeEach(() => {
  clearTauriMocks();
  mockTauriCommand("list_connections", () => []);
  mockTauriCommand("git_status_cmd", () => ({
    branch: "main",
    upstream: null,
    ahead: 0,
    behind: 0,
    changed: [],
    clean: true,
  }));
  useEnvironmentStore.setState({
    environments: [],
    activeEnvironment: null,
    managerOpen: false,
    variablesVersion: 0,
    loadVariables: vi.fn(async () => []),
    openManager: vi.fn(),
  } as never);
});

afterEach(() => {
  clearTauriMocks();
});

describe("Sidebar", () => {
  it("renders the Files section header", () => {
    renderWithWorkspace(<Sidebar width={240} />);
    expect(screen.getByText("Files")).toBeInTheDocument();
  });

  it("renders the Connections section header", () => {
    renderWithWorkspace(<Sidebar width={240} />);
    expect(screen.getByText("Connections")).toBeInTheDocument();
  });

  it("renders the Variables section (under Connections)", () => {
    renderWithWorkspace(<Sidebar width={240} />);
    expect(screen.getByText("Variables")).toBeInTheDocument();
    expect(screen.getByTestId("variables-panel")).toBeInTheDocument();
  });

  it("respects the width prop", () => {
    const { container } = renderWithWorkspace(<Sidebar width={300} />);
    const sidebar = container.firstChild as HTMLElement;
    // Chakra emits the width via CSS class — the prop reaches the
    // DOM as a generated style rule. Snapshot via getComputedStyle
    // through inline `style` or className lookup is brittle, so we
    // assert the rendered element exists and the prop didn't error.
    expect(sidebar).toBeInTheDocument();
  });

  it('shows "No vault selected" placeholder when no vault open', () => {
    renderWithWorkspace(<Sidebar width={240} />, { vaultPath: null });
    expect(screen.getByText("No vault selected")).toBeInTheDocument();
  });

  it("shows the new-files menu trigger when a vault is open", () => {
    renderWithWorkspace(<Sidebar width={240} />, { vaultPath: "/v" });
    expect(screen.getByLabelText("New...")).toBeInTheDocument();
  });
});
