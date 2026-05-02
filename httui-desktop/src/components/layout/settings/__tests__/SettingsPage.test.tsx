import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";

vi.mock("@/lib/theme/apply", () => ({ applyTheme: vi.fn() }));

import { SettingsPage } from "@/components/layout/settings/SettingsPage";
import { useSettingsStore } from "@/stores/settings";
import { useWorkspaceStore } from "@/stores/workspace";

beforeEach(() => {
  useSettingsStore.setState({
    settingsOpen: true,
    colorMode: "system",
    shortcutProfile: "default",
    density: "comfortable",
    vimEnabled: false,
    settings: {
      autoSaveMs: 1000,
      editorFontSize: 12,
      defaultFetchSize: 80,
      historyRetention: 10,
    },
    loaded: true,
  });
  useWorkspaceStore.setState({ vaultPath: "/tmp/vault" });
  clearTauriMocks();
  mockTauriCommand("get_user_config", () => ({
    version: "1",
    ui: {
      theme: "",
      font_family: "",
      font_size: 12,
      density: "comfortable",
      auto_save_ms: 1000,
      default_fetch_size: 80,
      history_retention: 10,
      vim_enabled: false,
      sidebar_open: true,
      color_mode: "system",
      mvp_migration_dismissed: false,
      hide_archived_in_quick_open: false,
      shortcut_profile: "default",
    },
    shortcuts: {},
    secrets: { backend: "auto", biometric: true, prompt_timeout_s: 60 },
    mcp: { servers: {} },
    active_envs: {},
  }));
  mockTauriCommand("set_user_config", () => undefined);
  mockTauriCommand("get_workspace_config_with_sources", () => ({
    defaults: {
      environment: "staging",
      git_remote: "origin",
      git_branch: "main",
      display_name: null,
    },
    sources: {
      environment: "workspace",
      git_remote: "workspace",
      git_branch: "workspace",
      display_name: "workspace",
    },
  }));
});

afterEach(() => {
  clearTauriMocks();
  useSettingsStore.setState({ settingsOpen: false });
  useWorkspaceStore.setState({ vaultPath: null });
});

describe("SettingsPage", () => {
  it("renders nothing when settingsOpen=false", () => {
    useSettingsStore.setState({ settingsOpen: false });
    renderWithProviders(<SettingsPage />);
    expect(screen.queryByTestId("settings-page")).toBeNull();
  });

  it("renders the two top-level tabs: User / Workspace", () => {
    renderWithProviders(<SettingsPage />);
    expect(screen.getByTestId("settings-tab-user")).toBeTruthy();
    expect(screen.getByTestId("settings-tab-workspace")).toBeTruthy();
  });

  it("opens with the User tab visible by default", () => {
    renderWithProviders(<SettingsPage />);
    expect(screen.getByTestId("settings-user-tab")).toBeTruthy();
  });

  it("clicking Workspace switches the visible tab", async () => {
    renderWithProviders(<SettingsPage />);
    await userEvent.setup().click(screen.getByTestId("settings-tab-workspace"));
    // Either the loaded tab shows up or the loading state — both
    // confirm the switch happened.
    const visible =
      screen.queryByTestId("settings-workspace-tab") ??
      screen.queryByTestId("settings-workspace-loading") ??
      screen.queryByTestId("settings-workspace-empty");
    expect(visible).not.toBeNull();
  });

  it("clicking the close button calls closeSettings", async () => {
    renderWithProviders(<SettingsPage />);
    await userEvent
      .setup()
      .click(screen.getByLabelText("Close settings"));
    expect(useSettingsStore.getState().settingsOpen).toBe(false);
  });

  it("clicking the backdrop closes the page", async () => {
    renderWithProviders(<SettingsPage />);
    await userEvent
      .setup()
      .click(screen.getByTestId("settings-page-backdrop"));
    expect(useSettingsStore.getState().settingsOpen).toBe(false);
  });
});
