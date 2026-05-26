import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";
import { DEFAULT_THEME } from "@/lib/theme/config";

// Mock applyTheme to avoid touching the real DOM
vi.mock("@/lib/theme/apply", () => ({
  applyTheme: vi.fn(),
}));

import { useSettingsStore } from "@/stores/settings";
import { applyTheme } from "@/lib/theme/apply";
import type { UserConfigFile, UserUiPrefs } from "@/lib/tauri/commands";

const DEFAULT_SETTINGS = {
  autoSaveMs: 1000,
  editorFontSize: 12,
  defaultFetchSize: 80,
  historyRetention: 10,
};

function userFile(uiOverrides: Partial<UserUiPrefs> = {}): UserConfigFile {
  return {
    version: "1",
    ui: {
      theme: "",
      font_family: "JetBrains Mono",
      font_size: 12,
      density: "comfortable",
      auto_save_ms: 1000,
      default_fetch_size: 80,
      history_retention: 10,
      vim_enabled: false,
      sidebar_open: true,
      color_mode: "system",
      mvp_migration_dismissed: false,
      ...uiOverrides,
    },
    shortcuts: {},
    secrets: { backend: "auto", biometric: true, prompt_timeout_s: 60 },
    mcp: { servers: {} },
    active_envs: {},
  };
}

function mockUserConfig(uiOverrides: Partial<UserUiPrefs> = {}) {
  let current = userFile(uiOverrides);
  mockTauriCommand("get_user_config", () => current);
  mockTauriCommand("set_user_config", (args) => {
    current = (args as { file: typeof current }).file;
  });
  return () => current;
}

function resetStore() {
  useSettingsStore.setState({
    settingsOpen: false,
    settings: DEFAULT_SETTINGS,
    loaded: false,
    theme: DEFAULT_THEME,
    colorMode: "system",
    vimEnabled: false,
    vimMode: "normal",
    sidebarOpen: true,
    gitSidePanelOpen: false,
    gitCommitTemplate: "",
    mvpMigrationDismissed: false,
    autoUpdateIncludePrereleases: false,
  });
}

// Wait for the patchUiPrefs read-modify-write round-trip.
// updateSetting fires-and-forgets; we need to flush both the
// `getUserConfig` await and the `setUserConfig` await.
async function flushPersist() {
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
}

describe("settingsStore", () => {
  beforeEach(() => {
    resetStore();
    clearTauriMocks();
    vi.mocked(applyTheme).mockClear();
  });

  afterEach(() => {
    clearTauriMocks();
  });

  describe("modal toggles", () => {
    it("openSettings/closeSettings flip the flag", () => {
      useSettingsStore.getState().openSettings();
      expect(useSettingsStore.getState().settingsOpen).toBe(true);
      useSettingsStore.getState().closeSettings();
      expect(useSettingsStore.getState().settingsOpen).toBe(false);
    });

    it("toggleSidebar / setSidebarOpen control sidebarOpen and persist", async () => {
      const read = mockUserConfig();
      useSettingsStore.getState().toggleSidebar();
      expect(useSettingsStore.getState().sidebarOpen).toBe(false);
      await flushPersist();
      expect(read().ui.sidebar_open).toBe(false);

      useSettingsStore.getState().setSidebarOpen(true);
      expect(useSettingsStore.getState().sidebarOpen).toBe(true);
      await flushPersist();
      expect(read().ui.sidebar_open).toBe(true);
    });

    it("toggleGitSidePanel / setGitSidePanelOpen control gitSidePanelOpen and persist", async () => {
      const read = mockUserConfig();
      useSettingsStore.getState().toggleGitSidePanel();
      expect(useSettingsStore.getState().gitSidePanelOpen).toBe(true);
      await flushPersist();
      expect(read().ui.git_side_panel_open).toBe(true);

      useSettingsStore.getState().setGitSidePanelOpen(false);
      expect(useSettingsStore.getState().gitSidePanelOpen).toBe(false);
      await flushPersist();
      expect(read().ui.git_side_panel_open).toBe(false);
    });

    it("setGitCommitTemplate updates state and persists", async () => {
      const read = mockUserConfig();
      useSettingsStore.getState().setGitCommitTemplate("docs: {{notes}}");
      expect(useSettingsStore.getState().gitCommitTemplate).toBe(
        "docs: {{notes}}",
      );
      await flushPersist();
      expect(read().ui.git_commit_template).toBe("docs: {{notes}}");
    });
  });

  describe("vim controls", () => {
    it("toggleVim flips vimEnabled and persists", async () => {
      const read = mockUserConfig();
      useSettingsStore.getState().toggleVim();
      expect(useSettingsStore.getState().vimEnabled).toBe(true);
      await flushPersist();
      expect(read().ui.vim_enabled).toBe(true);

      useSettingsStore.getState().toggleVim();
      expect(useSettingsStore.getState().vimEnabled).toBe(false);
      await flushPersist();
      expect(read().ui.vim_enabled).toBe(false);
    });

    it("setVimEnabled sets the flag and persists", async () => {
      const read = mockUserConfig();
      useSettingsStore.getState().setVimEnabled(true);
      expect(useSettingsStore.getState().vimEnabled).toBe(true);
      await flushPersist();
      expect(read().ui.vim_enabled).toBe(true);
    });

    it("setVimMode updates the mode (in-memory only)", () => {
      useSettingsStore.getState().setVimMode("insert");
      expect(useSettingsStore.getState().vimMode).toBe("insert");
    });
  });

  describe("updateSetting — persists to user.toml [ui]", () => {
    it("autoSaveMs writes ui.auto_save_ms", async () => {
      const read = mockUserConfig();
      useSettingsStore.getState().updateSetting("autoSaveMs", 500);
      expect(useSettingsStore.getState().settings.autoSaveMs).toBe(500);
      await flushPersist();
      expect(read().ui.auto_save_ms).toBe(500);
    });

    it("editorFontSize writes ui.font_size (note rename)", async () => {
      const read = mockUserConfig();
      useSettingsStore.getState().updateSetting("editorFontSize", 18);
      expect(useSettingsStore.getState().settings.editorFontSize).toBe(18);
      await flushPersist();
      expect(read().ui.font_size).toBe(18);
    });

    it("defaultFetchSize writes ui.default_fetch_size", async () => {
      const read = mockUserConfig();
      useSettingsStore.getState().updateSetting("defaultFetchSize", 200);
      expect(useSettingsStore.getState().settings.defaultFetchSize).toBe(200);
      await flushPersist();
      expect(read().ui.default_fetch_size).toBe(200);
    });

    it("historyRetention writes ui.history_retention", async () => {
      const read = mockUserConfig();
      useSettingsStore.getState().updateSetting("historyRetention", 25);
      expect(useSettingsStore.getState().settings.historyRetention).toBe(25);
      await flushPersist();
      expect(read().ui.history_retention).toBe(25);
    });
  });

  describe("theme", () => {
    it("updateTheme persists JSON of the merged config", async () => {
      const read = mockUserConfig();
      useSettingsStore.getState().updateTheme({ accentColor: "iris" });
      expect(useSettingsStore.getState().theme.accentColor).toBe("iris");
      await flushPersist();
      const stored = JSON.parse(read().ui.theme);
      expect(stored.accentColor).toBe("iris");
      expect(applyTheme).toHaveBeenCalled();
    });

    it("resetTheme reverts to DEFAULT_THEME and persists JSON", async () => {
      const read = mockUserConfig();
      useSettingsStore.getState().updateTheme({ accentColor: "iris" });
      await flushPersist();

      useSettingsStore.getState().resetTheme();
      expect(useSettingsStore.getState().theme).toEqual(DEFAULT_THEME);
      await flushPersist();
      const stored = JSON.parse(read().ui.theme);
      expect(stored.accentColor).toBe(DEFAULT_THEME.accentColor);
    });
  });

  describe("loadSettings", () => {
    it("populates settings + theme + vim/sidebar from user.toml [ui]", async () => {
      mockUserConfig({
        auto_save_ms: 750,
        font_size: 16,
        default_fetch_size: 150,
        history_retention: 25,
        vim_enabled: true,
        sidebar_open: false,
        theme: JSON.stringify({ accentColor: "ruby", grayTone: "slate" }),
      });

      await useSettingsStore.getState().loadSettings();

      const state = useSettingsStore.getState();
      expect(state.settings.autoSaveMs).toBe(750);
      expect(state.settings.editorFontSize).toBe(16);
      expect(state.settings.defaultFetchSize).toBe(150);
      expect(state.settings.historyRetention).toBe(25);
      expect(state.vimEnabled).toBe(true);
      expect(state.sidebarOpen).toBe(false);
      expect(state.theme.accentColor).toBe("ruby");
      expect(state.theme.grayTone).toBe("slate");
      expect(state.loaded).toBe(true);
      expect(applyTheme).toHaveBeenCalled();
    });

    it("hydrates gitSidePanelOpen from ui.git_side_panel_open", async () => {
      mockUserConfig({ git_side_panel_open: true });
      await useSettingsStore.getState().loadSettings();
      expect(useSettingsStore.getState().gitSidePanelOpen).toBe(true);
    });

    it("defaults gitSidePanelOpen to false when the key is absent", async () => {
      mockUserConfig({});
      await useSettingsStore.getState().loadSettings();
      expect(useSettingsStore.getState().gitSidePanelOpen).toBe(false);
    });

    it("hydrates gitCommitTemplate from ui.git_commit_template", async () => {
      mockUserConfig({ git_commit_template: "chore: {{count}} files" });
      await useSettingsStore.getState().loadSettings();
      expect(useSettingsStore.getState().gitCommitTemplate).toBe(
        "chore: {{count}} files",
      );
    });

    it("falls back to defaults for missing fields", async () => {
      mockUserConfig({
        // no overrides — ui defaults applied
      });

      await useSettingsStore.getState().loadSettings();

      const state = useSettingsStore.getState();
      // ui.auto_save_ms = 1000 in userFile() default
      expect(state.settings.autoSaveMs).toBe(1000);
      expect(state.theme).toEqual(DEFAULT_THEME);
    });

    it("falls back to DEFAULT_THEME when ui.theme is a bare string", async () => {
      // Migration writes a bare mode like "dark"; current ThemeConfig
      // has no `mode` field, so the bare string can't be merged in.
      // The store keeps DEFAULT_THEME and overwrites on next save.
      mockUserConfig({ theme: "dark" });

      await useSettingsStore.getState().loadSettings();

      expect(useSettingsStore.getState().theme).toEqual(DEFAULT_THEME);
    });

    it("falls back to DEFAULT_THEME when ui.theme is invalid JSON", async () => {
      mockUserConfig({ theme: "not valid json{{{" });

      await useSettingsStore.getState().loadSettings();

      expect(useSettingsStore.getState().theme).toEqual(DEFAULT_THEME);
    });
  });

  describe("colorMode", () => {
    it("defaults to 'system'", () => {
      expect(useSettingsStore.getState().colorMode).toBe("system");
    });

    it("setColorMode updates state and persists ui.color_mode", async () => {
      const read = mockUserConfig();

      useSettingsStore.getState().setColorMode("dark");

      expect(useSettingsStore.getState().colorMode).toBe("dark");
      await flushPersist();
      expect(read().ui.color_mode).toBe("dark");
    });

    it("loadSettings hydrates colorMode from ui.color_mode", async () => {
      mockUserConfig({ color_mode: "light" });

      await useSettingsStore.getState().loadSettings();

      expect(useSettingsStore.getState().colorMode).toBe("light");
    });

    it("falls back to 'system' when ui.color_mode is unrecognised", async () => {
      mockUserConfig({ color_mode: "high-contrast" });

      await useSettingsStore.getState().loadSettings();

      expect(useSettingsStore.getState().colorMode).toBe("system");
    });

    it("falls back to 'system' when ui.color_mode is missing", async () => {
      mockUserConfig({ color_mode: "" });

      await useSettingsStore.getState().loadSettings();

      expect(useSettingsStore.getState().colorMode).toBe("system");
    });
  });

  describe("mvpMigrationDismissed", () => {
    it("defaults to false", () => {
      expect(useSettingsStore.getState().mvpMigrationDismissed).toBe(false);
    });

    it("setMvpMigrationDismissed(true) updates state and persists", async () => {
      const read = mockUserConfig();

      useSettingsStore.getState().setMvpMigrationDismissed(true);

      expect(useSettingsStore.getState().mvpMigrationDismissed).toBe(true);
      await flushPersist();
      expect(read().ui.mvp_migration_dismissed).toBe(true);
    });

    it("setMvpMigrationDismissed(false) round-trips back to false", async () => {
      const read = mockUserConfig({ mvp_migration_dismissed: true });

      useSettingsStore.getState().setMvpMigrationDismissed(false);

      expect(useSettingsStore.getState().mvpMigrationDismissed).toBe(false);
      await flushPersist();
      expect(read().ui.mvp_migration_dismissed).toBe(false);
    });

    it("loadSettings hydrates mvpMigrationDismissed from config", async () => {
      mockUserConfig({ mvp_migration_dismissed: true });

      await useSettingsStore.getState().loadSettings();

      expect(useSettingsStore.getState().mvpMigrationDismissed).toBe(true);
    });

    it("falls back to false when ui.mvp_migration_dismissed is omitted", async () => {
      // Pass null instead of true/false to simulate a TOML file
      // written by an older version that pre-dates the field.
      mockUserConfig({ mvp_migration_dismissed: undefined });

      await useSettingsStore.getState().loadSettings();

      expect(useSettingsStore.getState().mvpMigrationDismissed).toBe(false);
    });
  });

  describe("autoUpdateIncludePrereleases", () => {
    it("defaults to false", () => {
      expect(useSettingsStore.getState().autoUpdateIncludePrereleases).toBe(
        false,
      );
    });

    it("setAutoUpdateIncludePrereleases(true) updates state and persists", async () => {
      const read = mockUserConfig();

      useSettingsStore.getState().setAutoUpdateIncludePrereleases(true);

      expect(useSettingsStore.getState().autoUpdateIncludePrereleases).toBe(
        true,
      );
      await flushPersist();
      expect(read().ui.auto_update_include_prereleases).toBe(true);
    });

    it("setAutoUpdateIncludePrereleases(false) round-trips back to false", async () => {
      const read = mockUserConfig({
        auto_update_include_prereleases: true,
      });

      useSettingsStore.getState().setAutoUpdateIncludePrereleases(false);

      expect(useSettingsStore.getState().autoUpdateIncludePrereleases).toBe(
        false,
      );
      await flushPersist();
      expect(read().ui.auto_update_include_prereleases).toBe(false);
    });

    it("loadSettings hydrates from ui.auto_update_include_prereleases", async () => {
      mockUserConfig({ auto_update_include_prereleases: true });

      await useSettingsStore.getState().loadSettings();

      expect(useSettingsStore.getState().autoUpdateIncludePrereleases).toBe(
        true,
      );
    });

    it("falls back to false when the key is omitted", async () => {
      mockUserConfig({ auto_update_include_prereleases: undefined });

      await useSettingsStore.getState().loadSettings();

      expect(useSettingsStore.getState().autoUpdateIncludePrereleases).toBe(
        false,
      );
    });
  });
});
