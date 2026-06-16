import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { renderWithProviders } from "@/test/render";

const listCrashLogs = vi.fn();
const readCrashLog = vi.fn();
const clearCrashLogs = vi.fn();
vi.mock("@/lib/tauri/crashes", () => ({
  listCrashLogs: () => listCrashLogs(),
  readCrashLog: (name: string) => readCrashLog(name),
  clearCrashLogs: () => clearCrashLogs(),
}));

import { CrashesSection } from "../CrashesSection";

const ROW = {
  name: "200-desktop.log",
  source: "desktop",
  epoch_ms: 200,
  summary: "thread panicked at boom",
};

beforeEach(() => {
  listCrashLogs.mockResolvedValue([]);
  readCrashLog.mockResolvedValue("thread panicked at boom\nbacktrace...");
  clearCrashLogs.mockResolvedValue(undefined);
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("CrashesSection", () => {
  it("shows the empty state when there are no crashes", async () => {
    renderWithProviders(<CrashesSection />);
    await waitFor(() => expect(listCrashLogs).toHaveBeenCalled());
    expect(screen.getByText("No crashes recorded")).toBeTruthy();
  });

  it("lists crash rows with their source and summary", async () => {
    listCrashLogs.mockResolvedValue([ROW]);
    renderWithProviders(<CrashesSection />);
    await waitFor(() =>
      expect(screen.getByText("thread panicked at boom")).toBeTruthy(),
    );
    expect(screen.getByText("desktop")).toBeTruthy();
  });

  it("reads and shows the body when a row is selected", async () => {
    const user = userEvent.setup();
    listCrashLogs.mockResolvedValue([ROW]);
    renderWithProviders(<CrashesSection />);
    await waitFor(() =>
      expect(screen.getByText("thread panicked at boom")).toBeTruthy(),
    );
    await user.click(screen.getByText("thread panicked at boom"));
    await waitFor(() => expect(readCrashLog).toHaveBeenCalledWith(ROW.name));
    expect(screen.getByText(/backtrace\.\.\./)).toBeTruthy();
  });

  it("clears all crash logs via the button", async () => {
    const user = userEvent.setup();
    listCrashLogs.mockResolvedValue([ROW]);
    renderWithProviders(<CrashesSection />);
    await waitFor(() => expect(listCrashLogs).toHaveBeenCalled());
    await user.click(screen.getByRole("button", { name: /clear all/i }));
    expect(clearCrashLogs).toHaveBeenCalledTimes(1);
  });
});
