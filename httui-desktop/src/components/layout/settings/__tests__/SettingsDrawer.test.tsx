import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { renderWithProviders } from "@/test/render";

// Stub each section so the drawer renders without booting stores / IPC.
// The unit under test is the tab composition + conditional render.
vi.mock("../GeneralSection", () => ({
  GeneralSection: () => <div data-testid="section-general" />,
}));
vi.mock("../ThemeSection", () => ({
  ThemeSection: () => <div data-testid="section-theme" />,
}));
vi.mock("../EditorSection", () => ({
  EditorSection: () => <div data-testid="section-editor" />,
}));
vi.mock("../ShortcutsSection", () => ({
  ShortcutsSection: () => <div data-testid="section-shortcuts" />,
}));
vi.mock("../UsageSection", () => ({
  UsageSection: () => <div data-testid="section-usage" />,
}));
vi.mock("../CrashesSection", () => ({
  CrashesSection: () => <div data-testid="section-crashes" />,
}));
vi.mock("../AuditSection", () => ({
  AuditSection: () => <div data-testid="section-audit" />,
}));
vi.mock("../AboutSection", () => ({
  AboutSection: () => <div data-testid="section-about" />,
}));

import { SettingsDrawer } from "../SettingsDrawer";
import { useSettingsStore } from "@/stores/settings";

beforeEach(() => {
  useSettingsStore.setState({ settingsOpen: true });
});

afterEach(() => {
  useSettingsStore.setState({ settingsOpen: false });
});

describe("SettingsDrawer", () => {
  it("renders nothing when closed", () => {
    useSettingsStore.setState({ settingsOpen: false });
    const { container } = renderWithProviders(<SettingsDrawer />);
    expect(container.querySelector("[data-testid]")).toBeNull();
  });

  it("opens on the General tab by default", () => {
    renderWithProviders(<SettingsDrawer />);
    expect(screen.getByTestId("section-general")).toBeTruthy();
    expect(screen.queryByTestId("section-usage")).toBeNull();
  });

  it("lists the Usage tab and switches to it on click", async () => {
    const user = userEvent.setup();
    renderWithProviders(<SettingsDrawer />);
    await user.click(screen.getByText("Usage"));
    expect(screen.getByTestId("section-usage")).toBeTruthy();
    expect(screen.queryByTestId("section-general")).toBeNull();
  });

  it("switches to the Crashes tab on click", async () => {
    const user = userEvent.setup();
    renderWithProviders(<SettingsDrawer />);
    await user.click(screen.getByText("Crashes"));
    expect(screen.getByTestId("section-crashes")).toBeTruthy();
  });

  it("switches to the Audit tab on click", async () => {
    const user = userEvent.setup();
    renderWithProviders(<SettingsDrawer />);
    await user.click(screen.getByText("Audit"));
    expect(screen.getByTestId("section-audit")).toBeTruthy();
  });

  it("closes via the close button", async () => {
    const user = userEvent.setup();
    renderWithProviders(<SettingsDrawer />);
    await user.click(screen.getByLabelText("Close"));
    expect(useSettingsStore.getState().settingsOpen).toBe(false);
  });
});
