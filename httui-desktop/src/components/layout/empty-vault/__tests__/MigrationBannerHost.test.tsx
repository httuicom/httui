import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { renderWithProviders, screen, act } from "@/test/render";
import userEvent from "@testing-library/user-event";

import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";
import { MigrationBannerHost } from "@/components/layout/empty-vault/MigrationBannerHost";
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

const REPORT = {
  vault_path: "/vault",
  backup_path: "/vault/notes.db.pre-v1-backup",
  connections_migrated: 2,
  connections_skipped: 0,
  environments_migrated: 1,
  environments_skipped: 0,
  variables_migrated: 5,
  variables_skipped: 0,
  prefs_migrated: 0,
  dry_run: false,
  notes: [],
};

beforeEach(() => {
  clearTauriMocks();
  resetSettings();
});

afterEach(() => {
  clearTauriMocks();
});

async function flush() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe("MigrationBannerHost", () => {
  it("renders nothing when the detection probe says nothing to do", async () => {
    mockTauriCommand("detect_vault_migration", () => ({
      has_legacy_db: false,
      has_v1_layout: true,
    }));
    renderWithProviders(<MigrationBannerHost vaultPath="/vault" />);
    await flush();
    expect(screen.queryByTestId("migration-banner")).toBeNull();
    expect(screen.queryByTestId("migration-banner-host")).toBeNull();
  });

  it("renders the banner when probe reports legacy db without v1 layout", async () => {
    mockTauriCommand("detect_vault_migration", () => ({
      has_legacy_db: true,
      has_v1_layout: false,
    }));
    renderWithProviders(<MigrationBannerHost vaultPath="/vault" />);
    await flush();
    expect(screen.getByTestId("migration-banner")).toBeInTheDocument();
  });

  it("hides the banner once the user has dismissed it", async () => {
    useSettingsStore.setState({ mvpMigrationDismissed: true });
    mockTauriCommand("detect_vault_migration", () => ({
      has_legacy_db: true,
      has_v1_layout: false,
    }));
    renderWithProviders(<MigrationBannerHost vaultPath="/vault" />);
    await flush();
    expect(screen.queryByTestId("migration-banner")).toBeNull();
  });

  it("Run migration → shows success banner with summary on resolve", async () => {
    let detectCalls = 0;
    mockTauriCommand("detect_vault_migration", () => {
      detectCalls += 1;
      // First probe: legacy → show banner. Refresh after migrate:
      // .httui/ now exists → no banner.
      return {
        has_legacy_db: true,
        has_v1_layout: detectCalls > 1,
      };
    });
    mockTauriCommand("migrate_vault_to_v1", () => REPORT);
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

    renderWithProviders(<MigrationBannerHost vaultPath="/vault" />);
    await flush();
    expect(screen.getByTestId("migration-banner")).toBeInTheDocument();

    await userEvent.setup().click(screen.getByTestId("migration-banner-run"));
    await flush();

    expect(screen.getByTestId("migration-success")).toBeInTheDocument();
    expect(screen.getByTestId("migration-success").textContent).toContain(
      "2 connection(s)",
    );
    expect(screen.getByTestId("migration-success").textContent).toContain(
      "1 environment(s)",
    );
    expect(screen.getByTestId("migration-success").textContent).toContain(
      "5 variable(s)",
    );
    // After refresh the banner is gone
    expect(screen.queryByTestId("migration-banner")).toBeNull();
  });

  it("Run migration → renders error banner when Tauri rejects", async () => {
    mockTauriCommand("detect_vault_migration", () => ({
      has_legacy_db: true,
      has_v1_layout: false,
    }));
    mockTauriCommand("migrate_vault_to_v1", () => {
      throw new Error("io: permission denied");
    });

    renderWithProviders(<MigrationBannerHost vaultPath="/vault" />);
    await flush();

    await userEvent.setup().click(screen.getByTestId("migration-banner-run"));
    await flush();

    const err = screen.getByTestId("migration-error");
    expect(err.textContent).toContain("Migration failed");
    expect(err.textContent).toContain("io: permission denied");
    // Banner stays visible — user may retry or dismiss
    expect(screen.getByTestId("migration-banner")).toBeInTheDocument();
  });

  it("Dismiss button hides the banner via the settings store", async () => {
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

    renderWithProviders(<MigrationBannerHost vaultPath="/vault" />);
    await flush();

    await userEvent
      .setup()
      .click(screen.getByLabelText("Dismiss migration banner"));
    await flush();

    expect(screen.queryByTestId("migration-banner")).toBeNull();
    expect(useSettingsStore.getState().mvpMigrationDismissed).toBe(true);
  });
});
