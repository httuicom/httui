import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderWithWorkspace, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";

vi.mock("@/lib/theme/apply", () => ({
  applyTheme: vi.fn(),
}));

import { GeneralSection } from "@/components/layout/settings/GeneralSection";
import { useSettingsStore } from "@/stores/settings";

beforeEach(() => {
  clearTauriMocks();
  mockTauriCommand("get_user_config", () => ({
    version: "1",
    ui: {
      theme: "",
      font_family: "",
      font_size: 14,
      density: "",
      auto_save_ms: 1000,
      default_fetch_size: 80,
      history_retention: 10,
      vim_enabled: false,
      sidebar_open: true,
      color_mode: "system",
    },
    shortcuts: {},
    secrets: { backend: "auto", biometric: true, prompt_timeout_s: 60 },
    mcp: { servers: {} },
    active_envs: {},
  }));
  mockTauriCommand("set_user_config", () => undefined);
  useSettingsStore.setState({
    settings: {
      autoSaveMs: 1000,
      editorFontSize: 14,
      defaultFetchSize: 80,
      historyRetention: 10,
    },
    colorMode: "system",
    loaded: true,
  });
});

afterEach(() => {
  clearTauriMocks();
});

describe("GeneralSection", () => {
  it("mounts the ColorModePicker (canvas-spec radio replaces legacy switch)", () => {
    renderWithWorkspace(<GeneralSection />);
    expect(screen.getByRole("radiogroup", { name: "Color mode" })).toBeTruthy();
  });

  it("renders the auto-save dropdown with the active option", () => {
    renderWithWorkspace(<GeneralSection />);
    const select = screen.getByRole("combobox") as HTMLSelectElement;
    expect(select.value).toBe("1000");
  });

  it("changing the auto-save dropdown writes through the store", async () => {
    renderWithWorkspace(<GeneralSection />);
    const select = screen.getByRole("combobox") as HTMLSelectElement;

    await userEvent.setup().selectOptions(select, "2000");

    expect(useSettingsStore.getState().settings.autoSaveMs).toBe(2000);
  });

  it("shows the autosave-disabled banner only when interval is 0", () => {
    useSettingsStore.setState({
      settings: {
        autoSaveMs: 0,
        editorFontSize: 14,
        defaultFetchSize: 80,
        historyRetention: 10,
      },
      colorMode: "system",
      loaded: true,
    });
    renderWithWorkspace(<GeneralSection />);
    expect(
      screen.getByText(/Auto-save is disabled\./i),
    ).toBeInTheDocument();
  });

  it("does not show the autosave-disabled banner when interval is positive", () => {
    renderWithWorkspace(<GeneralSection />);
    expect(screen.queryByText(/Auto-save is disabled/i)).toBeNull();
  });

  it("renders the history-retention input bound to the store", () => {
    renderWithWorkspace(<GeneralSection />);
    const input = screen.getByRole("spinbutton") as HTMLInputElement;
    expect(input.value).toBe("10");
  });

  it("history-retention input writes valid numbers through to the store", async () => {
    const { fireEvent } = await import("@testing-library/react");
    renderWithWorkspace(<GeneralSection />);
    const input = screen.getByRole("spinbutton") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "25" } });
    expect(useSettingsStore.getState().settings.historyRetention).toBe(25);
  });

  it("history-retention rejects out-of-range numbers", async () => {
    const { fireEvent } = await import("@testing-library/react");
    renderWithWorkspace(<GeneralSection />);
    const input = screen.getByRole("spinbutton") as HTMLInputElement;
    // 0 fails the `n > 0` guard; the store value stays at the seeded 10.
    fireEvent.change(input, { target: { value: "0" } });
    expect(useSettingsStore.getState().settings.historyRetention).toBe(10);
    // 101 fails the `n <= 100` guard.
    fireEvent.change(input, { target: { value: "101" } });
    expect(useSettingsStore.getState().settings.historyRetention).toBe(10);
    // NaN (empty / non-numeric) also rejected by Number.isFinite.
    fireEvent.change(input, { target: { value: "" } });
    expect(useSettingsStore.getState().settings.historyRetention).toBe(10);
  });

  it("renders 'None' when no vault is active", () => {
    renderWithWorkspace(<GeneralSection />, { vaultPath: null });
    // The Workspace section renders the literal "None" string.
    expect(screen.getByText("None")).toBeInTheDocument();
  });

  it("renders the active-vault path when present", () => {
    renderWithWorkspace(<GeneralSection />, { vaultPath: "/tmp/v" });
    expect(screen.getByText("/tmp/v")).toBeInTheDocument();
  });
});
