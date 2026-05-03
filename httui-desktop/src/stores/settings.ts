import { create } from "zustand";
import { devtools } from "zustand/middleware";
import { getUserConfig, setUserConfig } from "@/lib/tauri/commands";
import type { UserConfigFile, UserUiPrefs } from "@/lib/tauri/commands";
import type { ThemeConfig } from "@/lib/theme/config";
import { DEFAULT_THEME } from "@/lib/theme/config";
import { applyTheme } from "@/lib/theme/apply";

// --- Types ---

export interface AppSettings {
  autoSaveMs: number;
  editorFontSize: number;
  defaultFetchSize: number;
  /** Cap for HTTP block run history (per file/alias). Onda 3. */
  historyRetention: number;
}

const DEFAULTS: AppSettings = {
  autoSaveMs: 1000,
  editorFontSize: 12,
  defaultFetchSize: 80,
  historyRetention: 10,
};

/** Color mode contract (canvas §0): system | light | dark. Distinct
 * from the legacy `theme` ThemeConfig (accent / radius / density /
 * shadow customisation pending Epic 19 sweep). Wires to Chakra's
 * `next-themes` provider via `<ColorModeSync />`. */
export type ColorMode = "system" | "light" | "dark";

interface SettingsState {
  // Settings
  settingsOpen: boolean;
  settings: AppSettings;
  loaded: boolean;
  theme: ThemeConfig;
  colorMode: ColorMode;

  // Editor settings
  vimEnabled: boolean;
  vimMode: string;

  // Layout
  sidebarOpen: boolean;

  // MVP-to-v1 migration banner
  mvpMigrationDismissed: boolean;

  // Actions
  openSettings: () => void;
  closeSettings: () => void;
  updateSetting: <K extends keyof AppSettings>(
    key: K,
    value: AppSettings[K],
  ) => void;
  updateTheme: (partial: Partial<ThemeConfig>) => void;
  resetTheme: () => void;
  setColorMode: (mode: ColorMode) => void;
  toggleVim: () => void;
  setVimMode: (mode: string) => void;
  setVimEnabled: (enabled: boolean) => void;
  toggleSidebar: () => void;
  setSidebarOpen: (open: boolean) => void;
  setMvpMigrationDismissed: (dismissed: boolean) => void;
  loadSettings: () => Promise<void>;
}

const COLOR_MODE_VALUES: ReadonlySet<ColorMode> = new Set([
  "system",
  "light",
  "dark",
]);

function parseColorMode(raw: string | undefined): ColorMode {
  return raw && COLOR_MODE_VALUES.has(raw as ColorMode)
    ? (raw as ColorMode)
    : "system";
}

/**
 * Read-modify-write the `[ui]` section of `~/.config/httui/user.toml`.
 * Each call loads the whole file, applies `mutate` to the `ui`
 * subtree, and writes it back. Errors swallow at the call site
 * (we set in-memory state optimistically before persisting).
 */
async function patchUiPrefs(
  mutate: (ui: UserUiPrefs) => UserUiPrefs,
): Promise<void> {
  const current: UserConfigFile = await getUserConfig();
  const next: UserConfigFile = {
    ...current,
    ui: mutate(current.ui),
  };
  await setUserConfig(next);
}

// --- Store ---

export const useSettingsStore = create<SettingsState>()(
  devtools(
    (set) => ({
      settingsOpen: false,
      settings: DEFAULTS,
      loaded: false,
      theme: DEFAULT_THEME,
      colorMode: "system" as ColorMode,
      vimEnabled: false,
      vimMode: "normal",
      sidebarOpen: true,
      mvpMigrationDismissed: false,

      openSettings: () => set({ settingsOpen: true }),
      closeSettings: () => set({ settingsOpen: false }),

      updateSetting: (key, value) => {
        set((state) => ({
          settings: { ...state.settings, [key]: value },
        }));
        patchUiPrefs((ui) => ({
          ...ui,
          ...(key === "autoSaveMs" ? { auto_save_ms: value as number } : {}),
          ...(key === "editorFontSize" ? { font_size: value as number } : {}),
          ...(key === "defaultFetchSize"
            ? { default_fetch_size: value as number }
            : {}),
          ...(key === "historyRetention"
            ? { history_retention: value as number }
            : {}),
        })).catch(() => {});
      },

      updateTheme: (partial) => {
        set((state) => {
          const next = { ...state.theme, ...partial };
          applyTheme(next);
          patchUiPrefs((ui) => ({ ...ui, theme: JSON.stringify(next) })).catch(
            () => {},
          );
          return { theme: next };
        });
      },

      resetTheme: () => {
        set({ theme: DEFAULT_THEME });
        applyTheme(DEFAULT_THEME);
        patchUiPrefs((ui) => ({
          ...ui,
          theme: JSON.stringify(DEFAULT_THEME),
        })).catch(() => {});
      },

      setColorMode: (mode) => {
        set({ colorMode: mode });
        patchUiPrefs((ui) => ({ ...ui, color_mode: mode })).catch(() => {});
      },

      toggleVim: () =>
        set((state) => {
          const next = !state.vimEnabled;
          patchUiPrefs((ui) => ({ ...ui, vim_enabled: next })).catch(() => {});
          return { vimEnabled: next };
        }),
      setVimMode: (mode) => set({ vimMode: mode }),
      setVimEnabled: (enabled) => {
        set({ vimEnabled: enabled });
        patchUiPrefs((ui) => ({ ...ui, vim_enabled: enabled })).catch(() => {});
      },
      toggleSidebar: () =>
        set((state) => {
          const next = !state.sidebarOpen;
          patchUiPrefs((ui) => ({ ...ui, sidebar_open: next })).catch(() => {});
          return { sidebarOpen: next };
        }),
      setSidebarOpen: (open) => {
        set({ sidebarOpen: open });
        patchUiPrefs((ui) => ({ ...ui, sidebar_open: open })).catch(() => {});
      },

      setMvpMigrationDismissed: (dismissed) => {
        set({ mvpMigrationDismissed: dismissed });
        patchUiPrefs((ui) => ({
          ...ui,
          mvp_migration_dismissed: dismissed,
        })).catch(() => {});
      },

      loadSettings: async () => {
        const file = await getUserConfig();
        const ui = file.ui;

        // Theme is persisted as JSON of the full ThemeConfig. The
        // legacy migration (Story 03) writes a bare mode string —
        // since ThemeConfig has no `mode` field anymore (the v1
        // theme is structural: accentColor, density, shadow, …),
        // bare-string values fall through to DEFAULT_THEME and get
        // overwritten on the next save.
        let themeConfig = DEFAULT_THEME;
        const raw = ui.theme;
        if (raw) {
          try {
            const parsed = JSON.parse(raw);
            if (parsed && typeof parsed === "object") {
              themeConfig = { ...DEFAULT_THEME, ...parsed };
            }
          } catch {
            // bare string from migration — keep DEFAULT_THEME
          }
        }
        applyTheme(themeConfig);

        set({
          settings: {
            autoSaveMs: ui.auto_save_ms || DEFAULTS.autoSaveMs,
            editorFontSize: ui.font_size || DEFAULTS.editorFontSize,
            defaultFetchSize:
              ui.default_fetch_size || DEFAULTS.defaultFetchSize,
            historyRetention:
              ui.history_retention || DEFAULTS.historyRetention,
          },
          theme: themeConfig,
          colorMode: parseColorMode(ui.color_mode),
          vimEnabled: ui.vim_enabled ?? false,
          sidebarOpen: ui.sidebar_open ?? true,
          mvpMigrationDismissed: ui.mvp_migration_dismissed ?? false,
          loaded: true,
        });
      },
    }),
    { name: "settings-store" },
  ),
);
