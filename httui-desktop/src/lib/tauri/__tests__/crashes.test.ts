import { afterEach, describe, expect, it } from "vitest";

import {
  listCrashLogs,
  readCrashLog,
  clearCrashLogs,
} from "@/lib/tauri/crashes";
import { clearTauriMocks, mockTauriCommand } from "@/test/mocks/tauri";

afterEach(() => {
  clearTauriMocks();
});

describe("crashes Tauri wrappers", () => {
  it("listCrashLogs invokes 'list_crash_logs' and returns the rows", async () => {
    mockTauriCommand("list_crash_logs", () => [
      {
        name: "200-desktop.log",
        source: "desktop",
        epoch_ms: 200,
        summary: "boom",
      },
    ]);
    const rows = await listCrashLogs();
    expect(rows).toHaveLength(1);
    expect(rows[0].source).toBe("desktop");
  });

  it("readCrashLog forwards the name and returns the body", async () => {
    mockTauriCommand("read_crash_log", (args) => {
      expect(args).toEqual({ name: "200-desktop.log" });
      return "boom\nbacktrace";
    });
    expect(await readCrashLog("200-desktop.log")).toBe("boom\nbacktrace");
  });

  it("clearCrashLogs invokes 'clear_crash_logs'", async () => {
    let called = false;
    mockTauriCommand("clear_crash_logs", () => {
      called = true;
      return null;
    });
    await clearCrashLogs();
    expect(called).toBe(true);
  });
});
