import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { renderWithProviders } from "@/test/render";

const getFeatureUsage = vi.fn();
const clearFeatureUsage = vi.fn();
vi.mock("@/lib/tauri/telemetry", () => ({
  getFeatureUsage: (from: string, to: string) => getFeatureUsage(from, to),
  clearFeatureUsage: () => clearFeatureUsage(),
}));

import { UsageSection } from "../UsageSection";
import { useSettingsStore } from "@/stores/settings";

beforeEach(() => {
  getFeatureUsage.mockResolvedValue([]);
  clearFeatureUsage.mockResolvedValue(undefined);
  useSettingsStore.setState({ telemetryEnabled: false });
});

afterEach(() => {
  vi.clearAllMocks();
  useSettingsStore.setState({ telemetryEnabled: false });
});

describe("UsageSection", () => {
  it("shows the off-state hint when tracking is disabled and no data", async () => {
    renderWithProviders(<UsageSection />);
    await waitFor(() => expect(getFeatureUsage).toHaveBeenCalled());
    expect(screen.getByText(/Tracking is off/i)).toBeTruthy();
  });

  it("toggling the switch flips telemetryEnabled in the store", async () => {
    const user = userEvent.setup();
    renderWithProviders(<UsageSection />);
    const toggle = screen.getByLabelText("Record feature usage locally");
    await user.click(toggle);
    expect(useSettingsStore.getState().telemetryEnabled).toBe(true);
  });

  it("renders per-day totals from the fetched rows", async () => {
    getFeatureUsage.mockResolvedValue([
      { date: "2026-01-10", feature: "http_block_run", count: 3 },
      { date: "2026-01-10", feature: "db_block_run", count: 2 },
    ]);
    renderWithProviders(<UsageSection />);
    // Total for the day = 5; HTTP summary card shows 3, DB shows 2.
    await waitFor(() => expect(screen.getByText("5")).toBeTruthy());
    expect(screen.getByText("3")).toBeTruthy();
    expect(screen.getByText("2")).toBeTruthy();
  });

  it("clears usage data via the reset button", async () => {
    const user = userEvent.setup();
    getFeatureUsage.mockResolvedValue([
      { date: "2026-01-10", feature: "http_block_run", count: 1 },
    ]);
    renderWithProviders(<UsageSection />);
    await waitFor(() => expect(getFeatureUsage).toHaveBeenCalled());

    const clearBtn = screen.getByRole("button", { name: /clear usage data/i });
    await user.click(clearBtn);
    expect(clearFeatureUsage).toHaveBeenCalledTimes(1);
  });
});
