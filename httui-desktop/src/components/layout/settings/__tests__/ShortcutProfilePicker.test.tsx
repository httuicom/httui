import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";

vi.mock("@/lib/theme/apply", () => ({ applyTheme: vi.fn() }));

import { ShortcutProfilePicker } from "@/components/layout/settings/ShortcutProfilePicker";
import { useSettingsStore } from "@/stores/settings";

beforeEach(() => {
  useSettingsStore.setState({
    shortcutProfile: "default",
    vimEnabled: false,
    loaded: true,
  });
  clearTauriMocks();
  mockTauriCommand("get_user_config", () => ({
    version: "1",
    ui: {
      theme: "",
      font_family: "",
      font_size: 14,
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
});

afterEach(() => clearTauriMocks());

describe("ShortcutProfilePicker", () => {
  it("renders four pills with correct labels", () => {
    renderWithProviders(<ShortcutProfilePicker />);
    expect(screen.getByRole("radio", { name: "Default" })).toBeTruthy();
    expect(screen.getByRole("radio", { name: "Vim" })).toBeTruthy();
    expect(screen.getByRole("radio", { name: "VS Code" })).toBeTruthy();
    expect(screen.getByRole("radio", { name: "JetBrains" })).toBeTruthy();
  });

  it("marks vscode and jetbrains as aria-disabled", () => {
    renderWithProviders(<ShortcutProfilePicker />);
    expect(
      screen
        .getByRole("radio", { name: "VS Code" })
        .getAttribute("aria-disabled"),
    ).toBe("true");
    expect(
      screen
        .getByRole("radio", { name: "JetBrains" })
        .getAttribute("aria-disabled"),
    ).toBe("true");
    expect(
      screen
        .getByRole("radio", { name: "Default" })
        .getAttribute("aria-disabled"),
    ).toBe("false");
  });

  it("clicking Vim sets profile=vim AND vimEnabled=true", async () => {
    renderWithProviders(<ShortcutProfilePicker />);
    await userEvent.setup().click(screen.getByRole("radio", { name: "Vim" }));
    expect(useSettingsStore.getState().shortcutProfile).toBe("vim");
    expect(useSettingsStore.getState().vimEnabled).toBe(true);
  });

  it("clicking Default flips vimEnabled back to false", async () => {
    useSettingsStore.setState({
      shortcutProfile: "vim",
      vimEnabled: true,
      loaded: true,
    });
    renderWithProviders(<ShortcutProfilePicker />);
    await userEvent
      .setup()
      .click(screen.getByRole("radio", { name: "Default" }));
    expect(useSettingsStore.getState().shortcutProfile).toBe("default");
    expect(useSettingsStore.getState().vimEnabled).toBe(false);
  });

  it("clicking a disabled pill is a no-op", async () => {
    renderWithProviders(<ShortcutProfilePicker />);
    await userEvent
      .setup()
      .click(screen.getByRole("radio", { name: "VS Code" }));
    expect(useSettingsStore.getState().shortcutProfile).toBe("default");
  });

  it("active pill carries aria-checked=true and data-active='true'", () => {
    useSettingsStore.setState({
      shortcutProfile: "vim",
      vimEnabled: true,
      loaded: true,
    });
    renderWithProviders(<ShortcutProfilePicker />);
    const vim = screen.getByRole("radio", { name: "Vim" });
    expect(vim.getAttribute("aria-checked")).toBe("true");
    expect(vim.getAttribute("data-active")).toBe("true");
  });

  it("disabled pills carry the 'Coming soon' title attribute", () => {
    renderWithProviders(<ShortcutProfilePicker />);
    expect(
      screen.getByRole("radio", { name: "VS Code" }).getAttribute("title"),
    ).toBe("Coming soon");
  });
});
