import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";

vi.mock("@/lib/theme/apply", () => ({ applyTheme: vi.fn() }));

import { FontSizePicker } from "@/components/layout/settings/FontSizePicker";
import { useSettingsStore } from "@/stores/settings";

beforeEach(() => {
  useSettingsStore.setState({
    settings: {
      autoSaveMs: 1000,
      editorFontSize: 12,
      defaultFetchSize: 80,
      historyRetention: 10,
    },
    loaded: true,
  });
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
});

afterEach(() => clearTauriMocks());

describe("FontSizePicker", () => {
  it("renders the select with current font size", () => {
    renderWithProviders(<FontSizePicker />);
    const select = screen.getByLabelText(
      "Editor font size",
    ) as HTMLSelectElement;
    expect(select.value).toBe("12");
  });

  it("changing the select updates the store value", async () => {
    renderWithProviders(<FontSizePicker />);
    const select = screen.getByLabelText(
      "Editor font size",
    ) as HTMLSelectElement;
    await userEvent.setup().selectOptions(select, "14");
    expect(useSettingsStore.getState().settings.editorFontSize).toBe(14);
  });
});
