import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";

import { useMigrationDetection } from "@/hooks/useMigrationDetection";
import { useSettingsStore } from "@/stores/settings";
import { DEFAULT_THEME } from "@/lib/theme/config";

function resetSettings() {
  useSettingsStore.setState({
    settingsOpen: false,
    settings: {
      autoSaveMs: 1000,
      editorFontSize: 12,
      defaultFetchSize: 80,
      historyRetention: 10,
    },
    loaded: false,
    theme: DEFAULT_THEME,
    colorMode: "system",
    vimEnabled: false,
    vimMode: "normal",
    sidebarOpen: true,
    mvpMigrationDismissed: false,
  });
}

beforeEach(() => {
  clearTauriMocks();
  resetSettings();
});

afterEach(() => {
  clearTauriMocks();
});

describe("useMigrationDetection", () => {
  it("idles with null candidate when vaultPath is null", async () => {
    const { result } = renderHook(() => useMigrationDetection(null));
    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current.candidate).toBeNull();
    expect(result.current.shouldShowBanner).toBe(false);
  });

  it("populates candidate after the probe resolves", async () => {
    mockTauriCommand("detect_vault_migration", () => ({
      has_legacy_db: true,
      has_v1_layout: false,
    }));
    const { result } = renderHook(() => useMigrationDetection("/vault"));
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current.candidate).toEqual({
      has_legacy_db: true,
      has_v1_layout: false,
    });
    expect(result.current.shouldShowBanner).toBe(true);
  });

  it("hides the banner when v1 layout is already initialised", async () => {
    mockTauriCommand("detect_vault_migration", () => ({
      has_legacy_db: true,
      has_v1_layout: true,
    }));
    const { result } = renderHook(() => useMigrationDetection("/vault"));
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current.shouldShowBanner).toBe(false);
  });

  it("hides the banner when only legacy db is missing", async () => {
    mockTauriCommand("detect_vault_migration", () => ({
      has_legacy_db: false,
      has_v1_layout: false,
    }));
    const { result } = renderHook(() => useMigrationDetection("/vault"));
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current.shouldShowBanner).toBe(false);
  });

  it("hides the banner when the user has dismissed it", async () => {
    useSettingsStore.setState({ mvpMigrationDismissed: true });
    mockTauriCommand("detect_vault_migration", () => ({
      has_legacy_db: true,
      has_v1_layout: false,
    }));
    const { result } = renderHook(() => useMigrationDetection("/vault"));
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current.candidate).toEqual({
      has_legacy_db: true,
      has_v1_layout: false,
    });
    expect(result.current.shouldShowBanner).toBe(false);
  });

  it("dismiss() flips the user pref and hides the banner", async () => {
    mockTauriCommand("detect_vault_migration", () => ({
      has_legacy_db: true,
      has_v1_layout: false,
    }));
    mockTauriCommand("get_user_config", () => ({
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
      },
      shortcuts: {},
      secrets: { backend: "auto", biometric: true, prompt_timeout_s: 60 },
      mcp: { servers: {} },
      active_envs: {},
    }));
    mockTauriCommand("set_user_config", () => undefined);

    const { result } = renderHook(() => useMigrationDetection("/vault"));
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current.shouldShowBanner).toBe(true);

    await act(async () => {
      result.current.dismiss();
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current.shouldShowBanner).toBe(false);
    expect(useSettingsStore.getState().mvpMigrationDismissed).toBe(true);
  });

  it("refresh() forces a re-probe", async () => {
    let calls = 0;
    mockTauriCommand("detect_vault_migration", () => {
      calls += 1;
      return { has_legacy_db: calls === 1, has_v1_layout: false };
    });

    const { result } = renderHook(() => useMigrationDetection("/vault"));
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(calls).toBe(1);
    expect(result.current.shouldShowBanner).toBe(true);

    await act(async () => {
      result.current.refresh();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(calls).toBe(2);
    // After refresh has_legacy_db is false → no banner
    expect(result.current.shouldShowBanner).toBe(false);
  });

  it("swallows probe errors and leaves candidate at null", async () => {
    mockTauriCommand("detect_vault_migration", () => {
      throw new Error("io error");
    });
    const { result } = renderHook(() => useMigrationDetection("/vault"));
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current.candidate).toBeNull();
    expect(result.current.shouldShowBanner).toBe(false);
  });
});
