import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderWithProviders } from "@/test/render";

// Stub every heavy child so AppShell renders without booting CM6,
// chat sidecar, Tauri bridge, etc. The unit under test is the
// composition / conditional-render logic — children get their own
// tests in their own files.
vi.mock("@/components/layout/TopBar", () => ({
  TopBar: () => <div data-testid="topbar" />,
}));
vi.mock("@/components/layout/Sidebar", () => ({
  Sidebar: () => <div data-testid="sidebar" />,
}));
vi.mock("@/components/layout/StatusBar", () => ({
  StatusBar: () => <div data-testid="statusbar" />,
}));
vi.mock("@/components/layout/pane", () => ({
  PaneContainer: () => <div data-testid="pane-container" />,
}));
vi.mock("@/components/search/QuickOpen", () => ({
  QuickOpen: () => <div data-testid="quick-open" />,
}));
vi.mock("@/components/search/SearchPanel", () => ({
  SearchPanel: () => <div data-testid="search-panel" />,
}));
vi.mock("@/components/layout/environments/EnvironmentManager", () => ({
  EnvironmentManager: () => <div data-testid="env-manager" />,
}));
vi.mock("@/components/layout/settings/SettingsDrawer", () => ({
  SettingsDrawer: () => <div data-testid="settings-drawer" />,
}));
vi.mock("@/components/layout/schema/SchemaPanel", () => ({
  SchemaPanel: () => <div data-testid="schema-panel" />,
}));
vi.mock("@/components/chat/ChatPanel", () => ({
  ChatPanel: () => <div data-testid="chat-panel" />,
}));
vi.mock("@/components/layout/git/GitSidePanel", () => ({
  GitSidePanel: () => <div data-testid="git-side-panel" />,
}));
vi.mock("@/components/layout/EmptyVaultScreen", () => ({
  EmptyVaultScreen: () => <div data-testid="empty-vault" />,
}));

const colorModeSyncMounts = vi.fn();
vi.mock("@/components/layout/ColorModeSync", () => ({
  ColorModeSync: () => {
    colorModeSyncMounts();
    return <div data-testid="color-mode-sync" />;
  },
}));

vi.mock("@/stores/tauri-bridge", () => ({ initTauriBridge: vi.fn() }));
vi.mock("@/hooks/useFileOperations", () => ({
  useFileOperations: () => ({
    inlineCreate: null,
    handleStartCreate: vi.fn(),
    handleCreateNote: vi.fn(),
    handleCreateFolder: vi.fn(),
    handleRename: vi.fn(),
    handleDelete: vi.fn(),
    handleMoveFile: vi.fn(),
    cancelInlineCreate: vi.fn(),
  }),
}));
vi.mock("@/hooks/useEditorSession", () => ({
  useEditorSession: () => ({ handleFileSelect: vi.fn() }),
}));
vi.mock("@/hooks/useKeyboardShortcuts", () => ({
  useKeyboardShortcuts: vi.fn(),
}));
vi.mock("@/hooks/useSidebarResize", () => ({
  useSidebarResize: () => ({
    sidebarWidth: 240,
    isResizing: false,
    handleMouseDown: vi.fn(),
  }),
}));
vi.mock("@/hooks/useSessionPersistence", () => ({
  useSessionPersistence: vi.fn(),
}));
vi.mock("@/hooks/useAutoUpdate", () => ({
  useAutoUpdate: vi.fn(),
}));
vi.mock("@/hooks/useSecretEnvKeysSync", () => ({
  useSecretEnvKeysSync: vi.fn(),
}));

import { AppShell } from "@/components/layout/AppShell";
import { useWorkspaceStore } from "@/stores/workspace";
import { useSettingsStore } from "@/stores/settings";

beforeEach(() => {
  colorModeSyncMounts.mockClear();
  useWorkspaceStore.setState({
    vaultPath: null,
    vaults: [],
    entries: [],
  } as never);
});

afterEach(() => {
  useWorkspaceStore.setState({
    vaultPath: null,
    vaults: [],
    entries: [],
  } as never);
  useSettingsStore.setState({ gitSidePanelOpen: false } as never);
});

describe("AppShell", () => {
  it("mounts ColorModeSync once at the root (theme bridge)", () => {
    renderWithProviders(<AppShell />);
    expect(colorModeSyncMounts).toHaveBeenCalledTimes(1);
  });

  it("renders the empty-vault screen when no vault is open", () => {
    useWorkspaceStore.setState({
      vaultPath: null,
      vaults: [],
      entries: [],
    } as never);
    const { getByTestId, queryByTestId } = renderWithProviders(<AppShell />);
    expect(getByTestId("empty-vault")).toBeTruthy();
    // PaneContainer (the editor surface) should NOT mount when there
    // is no vault.
    expect(queryByTestId("pane-container")).toBeNull();
  });

  it("renders the editor surface (TopBar + Sidebar + Pane + Status) when a vault is active", () => {
    useWorkspaceStore.setState({
      vaultPath: "/v",
      vaults: [],
      entries: [],
    } as never);
    const { getByTestId, queryByTestId } = renderWithProviders(<AppShell />);
    expect(getByTestId("topbar")).toBeTruthy();
    expect(getByTestId("sidebar")).toBeTruthy();
    expect(getByTestId("pane-container")).toBeTruthy();
    expect(getByTestId("statusbar")).toBeTruthy();
    expect(queryByTestId("empty-vault")).toBeNull();
  });

  it("does not render the right-panel overlay when no right panel is open", () => {
    useWorkspaceStore.setState({
      vaultPath: "/v",
      vaults: [],
      entries: [],
    } as never);
    const { queryByTestId } = renderWithProviders(<AppShell />);
    expect(queryByTestId("right-panel-overlay")).toBeNull();
  });

  it("mounts open right panels inside an absolutely-positioned overlay (floats over the editor instead of pushing it)", () => {
    useWorkspaceStore.setState({
      vaultPath: "/v",
      vaults: [],
      entries: [],
    } as never);
    useSettingsStore.setState({ gitSidePanelOpen: true } as never);

    const { getByTestId } = renderWithProviders(<AppShell />);

    const overlay = getByTestId("right-panel-overlay");
    // The panel renders inside the overlay, not as a flex sibling of
    // the editor surface — so it can't steal width from PaneContainer.
    expect(overlay.contains(getByTestId("git-side-panel"))).toBe(true);
    // Pulled out of the flex flow so the editor keeps its full width.
    expect(getComputedStyle(overlay).position).toBe("absolute");
  });
});
