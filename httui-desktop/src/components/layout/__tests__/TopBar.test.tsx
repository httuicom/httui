import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderWithWorkspace, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";

import { TopBar } from "@/components/layout/TopBar";
import { useEnvironmentStore } from "@/stores/environment";
import { useSettingsStore } from "@/stores/settings";
import { usePaneStore } from "@/stores/pane";
import { clearTauriMocks } from "@/test/mocks/tauri";

vi.mock("@/lib/theme/apply", () => ({ applyTheme: vi.fn() }));

const mkEnv = (id: string, name: string, isActive = false) => ({
  id,
  name,
  is_active: isActive,
  created_at: "2026-01-01T00:00:00Z",
});

const baseProps = {
  sidebarOpen: true,
  onToggleSidebar: vi.fn(),
  chatOpen: false,
  onToggleChat: vi.fn(),
  schemaPanelOpen: false,
  onToggleSchemaPanel: vi.fn(),
};

describe("TopBar", () => {
  beforeEach(() => {
    clearTauriMocks();
    useEnvironmentStore.setState({
      environments: [],
      activeEnvironment: null,
      managerOpen: false,
      variablesVersion: 0,
      switchEnvironment: vi.fn(),
    } as never);
    useSettingsStore.setState({ settingsOpen: false });
    usePaneStore.setState({
      layout: { type: "leaf", id: "p1", tabs: [], activeTab: 0 },
      activePaneId: "p1",
      unsavedFiles: new Set(),
    } as never);
  });

  afterEach(() => {
    clearTauriMocks();
  });

  describe("layout shape", () => {
    it("renders the httui brand image (canvas §4)", () => {
      renderWithWorkspace(<TopBar {...baseProps} />);
      expect(screen.getByAltText("httui")).toBeInTheDocument();
    });

    it("renders 'no vault' breadcrumb fallback when no vault is open", () => {
      renderWithWorkspace(<TopBar {...baseProps} />, { vaultPath: null });
      expect(screen.getByText("no vault")).toBeInTheDocument();
    });

    it("renders the vault basename as the workspace segment", () => {
      renderWithWorkspace(<TopBar {...baseProps} />, {
        vaultPath: "/Users/me/notes-vault",
      });
      expect(screen.getByText("notes-vault")).toBeInTheDocument();
    });

    it("does not render the segmented env switcher (moved to StatusBar in V2)", () => {
      useEnvironmentStore.setState({
        environments: [mkEnv("a", "local"), mkEnv("b", "prod")],
        activeEnvironment: mkEnv("a", "local"),
        switchEnvironment: vi.fn(),
      } as never);
      renderWithWorkspace(<TopBar {...baseProps} />);
      // Old segmented switcher used role=tab; the env interaction
      // now lives in the status bar.
      expect(screen.queryAllByRole("tab")).toHaveLength(0);
    });

    it("renders the search ⌘K placeholder", () => {
      renderWithWorkspace(<TopBar {...baseProps} />);
      expect(
        screen.getByLabelText("Search blocks, vars, schema"),
      ).toBeInTheDocument();
      expect(screen.getByText("⌘K")).toBeInTheDocument();
    });

    it("renders the branch button (read-only label awaiting V10)", () => {
      renderWithWorkspace(<TopBar {...baseProps} />);
      expect(screen.getByLabelText("Switch branch")).toBeInTheDocument();
    });

    it("does not render a Run-all button (dropped 2026-05-01 / V2 cenário 1)", () => {
      renderWithWorkspace(<TopBar {...baseProps} />);
      expect(
        screen.queryByLabelText("Run all blocks in document"),
      ).not.toBeInTheDocument();
      expect(screen.queryByText("Run all")).not.toBeInTheDocument();
    });
  });

  describe("toggle controls (right edge)", () => {
    it("toggle sidebar dispatches onToggleSidebar", async () => {
      const user = userEvent.setup();
      const onToggleSidebar = vi.fn();
      renderWithWorkspace(
        <TopBar {...baseProps} onToggleSidebar={onToggleSidebar} />,
      );

      await user.click(screen.getByRole("button", { name: /hide sidebar/i }));
      expect(onToggleSidebar).toHaveBeenCalledTimes(1);
    });

    it("aria-label flips when sidebar is closed", () => {
      renderWithWorkspace(<TopBar {...baseProps} sidebarOpen={false} />);
      expect(
        screen.getByRole("button", { name: /show sidebar/i }),
      ).toBeInTheDocument();
    });

    it("chat button calls onToggleChat", async () => {
      const user = userEvent.setup();
      const onToggleChat = vi.fn();
      renderWithWorkspace(<TopBar {...baseProps} onToggleChat={onToggleChat} />);
      await user.click(screen.getByRole("button", { name: /open chat/i }));
      expect(onToggleChat).toHaveBeenCalledTimes(1);
    });

    it("schema panel button reflects open state in aria-label", () => {
      renderWithWorkspace(<TopBar {...baseProps} schemaPanelOpen={true} />);
      expect(
        screen.getByRole("button", { name: /close schema panel/i }),
      ).toBeInTheDocument();
    });

    it("schema panel button calls onToggleSchemaPanel", async () => {
      const user = userEvent.setup();
      const onToggleSchemaPanel = vi.fn();
      renderWithWorkspace(
        <TopBar {...baseProps} onToggleSchemaPanel={onToggleSchemaPanel} />,
      );
      await user.click(
        screen.getByRole("button", { name: /open schema panel/i }),
      );
      expect(onToggleSchemaPanel).toHaveBeenCalledTimes(1);
    });

    it("settings button opens the settings store flag", async () => {
      const user = userEvent.setup();
      renderWithWorkspace(<TopBar {...baseProps} />);

      await user.click(screen.getByRole("button", { name: /settings/i }));

      expect(useSettingsStore.getState().settingsOpen).toBe(true);
    });
  });

  describe("search + branch + breadcrumb wiring", () => {
    it("clicking the search trigger dispatches the supplied onSearch", async () => {
      const user = userEvent.setup();
      const onSearch = vi.fn();
      renderWithWorkspace(<TopBar {...baseProps} onSearch={onSearch} />);
      await user.click(
        screen.getByLabelText("Search blocks, vars, schema"),
      );
      expect(onSearch).toHaveBeenCalledTimes(1);
    });

    it("defaultSearchTrigger fires a synthetic Cmd+P when onSearch is not supplied", async () => {
      const user = userEvent.setup();
      const dispatch = vi.spyOn(window, "dispatchEvent");
      renderWithWorkspace(<TopBar {...baseProps} />);

      await user.click(
        screen.getByLabelText("Search blocks, vars, schema"),
      );

      const calls = dispatch.mock.calls.filter(
        (c) => (c[0] as KeyboardEvent).key === "p",
      );
      expect(calls.length).toBeGreaterThanOrEqual(1);
      const ev = calls[0][0] as KeyboardEvent;
      expect(ev.metaKey).toBe(true);
      dispatch.mockRestore();
    });

    it("branch label reflects gitStatus.branch when a vault is open", async () => {
      const { mockTauriCommand } = await import("@/test/mocks/tauri");
      mockTauriCommand("git_status_cmd", () => ({
        branch: "feat/login",
        upstream: null,
        ahead: 0,
        behind: 0,
        changed: [],
        clean: true,
      }));

      const { findByLabelText } = renderWithWorkspace(
        <TopBar {...baseProps} />,
        { vaultPath: "/v" },
      );
      const btn = await findByLabelText("Switch branch");
      // useGitStatus polls async; allow a microtask flush.
      await new Promise((r) => setTimeout(r, 0));
      expect(btn.textContent).toContain("feat/login");
    });

    it("branch label falls back to 'main' when no gitStatus has resolved yet", () => {
      renderWithWorkspace(<TopBar {...baseProps} />, { vaultPath: null });
      expect(
        screen.getByLabelText("Switch branch").textContent,
      ).toContain("main");
    });

    it("workspace segment opens a vault picker dropdown (not a cycle)", async () => {
      const user = userEvent.setup();
      const switchVault = vi.fn(async () => {});
      renderWithWorkspace(<TopBar {...baseProps} />, {
        vaultPath: "/v1",
        vaults: ["/v1", "/v2"],
        switchVault,
      });

      // Trigger renders as a button labelled by workspace name.
      const trigger = screen.getByRole("button", { name: /Workspace v1/ });
      await user.click(trigger);

      // Dropdown lists every vault and the "Open other vault…" item.
      expect(screen.getByText("/v1")).toBeInTheDocument();
      expect(screen.getByText("/v2")).toBeInTheDocument();
      expect(screen.getByText("Abrir outro vault…")).toBeInTheDocument();

      // Picking the inactive vault triggers switchVault — no cycling.
      await user.click(screen.getByText("v2"));
      expect(switchVault).toHaveBeenCalledWith("/v2");
    });

    it("breadcrumb shows the active tab path with dirty dot when unsaved", () => {
      usePaneStore.setState({
        layout: {
          type: "leaf",
          id: "p1",
          tabs: [
            {
              filePath: "/v/runbooks/auth/login.md",
              vaultPath: "/v",
              unsaved: true,
            },
          ],
          activeTab: 0,
        },
        activePaneId: "p1",
        unsavedFiles: new Set(["/v/runbooks/auth/login.md"]),
      } as never);
      renderWithWorkspace(<TopBar {...baseProps} />, { vaultPath: "/v" });
      expect(screen.getByText("auth")).toBeInTheDocument();
      expect(screen.getByText("login.md")).toBeInTheDocument();
      expect(screen.getByTestId("dirty-indicator")).toBeInTheDocument();
    });
  });
});
