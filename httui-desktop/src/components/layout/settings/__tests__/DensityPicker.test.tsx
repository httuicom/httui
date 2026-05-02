import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";

vi.mock("@/lib/theme/apply", () => ({ applyTheme: vi.fn() }));

import { DensityPicker } from "@/components/layout/settings/DensityPicker";
import { useSettingsStore } from "@/stores/settings";

beforeEach(() => {
  useSettingsStore.setState({ density: "comfortable", loaded: true });
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

describe("DensityPicker", () => {
  it("renders three options", () => {
    renderWithProviders(<DensityPicker />);
    expect(screen.getByRole("radio", { name: "Compact" })).toBeTruthy();
    expect(screen.getByRole("radio", { name: "Comfortable" })).toBeTruthy();
    expect(screen.getByRole("radio", { name: "Spacious" })).toBeTruthy();
  });

  it("clicking Compact updates store + writes CSS var", async () => {
    renderWithProviders(<DensityPicker />);
    await userEvent
      .setup()
      .click(screen.getByRole("radio", { name: "Compact" }));
    expect(useSettingsStore.getState().density).toBe("compact");
    expect(
      document.documentElement.style.getPropertyValue("--httui-density"),
    ).toBe("0.85");
  });

  it("comfortable resets the CSS scale to 1", async () => {
    document.documentElement.style.setProperty("--httui-density", "0.85");
    renderWithProviders(<DensityPicker />);
    await userEvent
      .setup()
      .click(screen.getByRole("radio", { name: "Comfortable" }));
    expect(
      document.documentElement.style.getPropertyValue("--httui-density"),
    ).toBe("1");
  });

  it("active option carries aria-checked=true", () => {
    useSettingsStore.setState({ density: "spacious", loaded: true });
    renderWithProviders(<DensityPicker />);
    expect(
      screen.getByRole("radio", { name: "Spacious" }).getAttribute("aria-checked"),
    ).toBe("true");
  });
});
