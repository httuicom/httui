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

/** Keyboard-shortcut profile selector (V3 cenário 1). `default` and
 * `vim` are functional; `vscode` and `jetbrains` are surfaced disabled
 * in the UI ("coming soon") and resolve to `default` at runtime. */
export type ShortcutProfile = "default" | "vim" | "vscode" | "jetbrains";

const SHORTCUT_PROFILE_VALUES: ReadonlySet<ShortcutProfile> = new Set([
  "default",
  "vim",
  "vscode",
  "jetbrains",
]);

function parseShortcutProfile(raw: string | undefined): ShortcutProfile {
  return raw && SHORTCUT_PROFILE_VALUES.has(raw as ShortcutProfile)
    ? (raw as ShortcutProfile)
    : "default";
}

/** UI density (V3 cenário 1.2). `comfortable` is the baseline; the
 * other two scale a CSS custom property `--httui-density` consumed by
 * components that want denser spacing. */
export type Density = "compact" | "comfortable" | "spacious";

const DENSITY_VALUES: ReadonlySet<Density> = new Set([
  "compact",
  "comfortable",
  "spacious",
]);

function parseDensity(raw: string | undefined): Density {
  return raw && DENSITY_VALUES.has(raw as Density)
    ? (raw as Density)
    : "comfortable";
}

const DENSITY_SCALE: Record<Density, string> = {
  compact: "0.85",
  comfortable: "1",
  spacious: "1.15",
};

function applyDensity(d: Density) {
  if (typeof document === "undefined") return;
  document.documentElement.style.setProperty(
    "--httui-density",
    DENSITY_SCALE[d],
  );
}

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

  // V3 — Settings UI
  shortcutProfile: ShortcutProfile;
  density: Density;

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
  /** Set the shortcut profile. `vim` maps `vimEnabled=true`;
   * `default` maps `vimEnabled=false`. `vscode`/`jetbrains` are
   * coming-soon stubs — the picker should surface them disabled. */
  setShortcutProfile: (profile: ShortcutProfile) => void;
  setDensity: (density: Density) => void;
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
      shortcutProfile: "default" as ShortcutProfile,
      density: "comfortable" as Density,

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

      setShortcutProfile: (profile) => {
        // `vim` ↔ vimEnabled true; `default` ↔ false. The other two
        // are surfaced disabled in the UI but if persisted by hand
        // they resolve to default behavior.
        const vimEnabled = profile === "vim";
        set({ shortcutProfile: profile, vimEnabled });
        patchUiPrefs((ui) => ({
          ...ui,
          shortcut_profile: profile,
          vim_enabled: vimEnabled,
        })).catch(() => {});
      },

      setDensity: (density) => {
        set({ density });
        applyDensity(density);
        patchUiPrefs((ui) => ({ ...ui, density })).catch(() => {});
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

        const density = parseDensity(ui.density);
        applyDensity(density);
        const profile = parseShortcutProfile(ui.shortcut_profile);
        // Honor stored vim_enabled but reconcile when profile says
        // otherwise — the profile is the source of truth in V3.
        const vimEnabled = profile === "vim" ? true : (ui.vim_enabled ?? false);

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
          vimEnabled,
          sidebarOpen: ui.sidebar_open ?? true,
          mvpMigrationDismissed: ui.mvp_migration_dismissed ?? false,
          shortcutProfile: profile,
          density,
          loaded: true,
        });
      },
    }),
    { name: "settings-store" },
  ),
);
