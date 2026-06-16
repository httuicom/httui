import { afterEach, beforeEach, describe, expect, it } from "vitest";

import {
  recordFeatureUsage,
  getFeatureUsage,
  clearFeatureUsage,
} from "@/lib/tauri/telemetry";
import { useSettingsStore } from "@/stores/settings";
import { clearTauriMocks, mockTauriCommand } from "@/test/mocks/tauri";

beforeEach(() => {
  useSettingsStore.setState({ telemetryEnabled: false });
});

afterEach(() => {
  clearTauriMocks();
  useSettingsStore.setState({ telemetryEnabled: false });
});

describe("recordFeatureUsage", () => {
  it("does NOT invoke when telemetry is opted out", async () => {
    let called = false;
    mockTauriCommand("record_feature_usage", () => {
      called = true;
      return null;
    });

    recordFeatureUsage("http_block_run");
    // Give any (incorrectly scheduled) microtask a chance to run.
    await Promise.resolve();
    expect(called).toBe(false);
  });

  it("invokes with the feature name when opted in", async () => {
    let captured: unknown = null;
    mockTauriCommand("record_feature_usage", (args) => {
      captured = args;
      return null;
    });

    useSettingsStore.setState({ telemetryEnabled: true });
    recordFeatureUsage("db_block_run");
    await Promise.resolve();
    expect(captured).toEqual({ feature: "db_block_run" });
  });

  it("swallows a backend error so a block run is never disrupted", async () => {
    mockTauriCommand("record_feature_usage", () => {
      throw new Error("db locked");
    });
    useSettingsStore.setState({ telemetryEnabled: true });
    // Must not throw synchronously.
    expect(() => recordFeatureUsage("http_block_run")).not.toThrow();
    await Promise.resolve();
  });
});

describe("getFeatureUsage / clearFeatureUsage", () => {
  it("getFeatureUsage forwards the date range and returns rows", async () => {
    mockTauriCommand("get_feature_usage", (args) => {
      expect(args).toEqual({ from: "2026-01-01", to: "2026-01-31" });
      return [{ date: "2026-01-10", feature: "http_block_run", count: 3 }];
    });
    const rows = await getFeatureUsage("2026-01-01", "2026-01-31");
    expect(rows).toEqual([
      { date: "2026-01-10", feature: "http_block_run", count: 3 },
    ]);
  });

  it("clearFeatureUsage invokes the clear command", async () => {
    let called = false;
    mockTauriCommand("clear_feature_usage", () => {
      called = true;
      return null;
    });
    await clearFeatureUsage();
    expect(called).toBe(true);
  });
});
