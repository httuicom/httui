import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderHook } from "@testing-library/react";

const check = vi.fn();
const ask = vi.fn();
const relaunch = vi.fn();

vi.mock("@tauri-apps/plugin-updater", () => ({
  check: () => check(),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  ask: (msg: string, opts: unknown) => ask(msg, opts),
}));
vi.mock("@tauri-apps/plugin-process", () => ({
  relaunch: () => relaunch(),
}));

import { useAutoUpdate } from "@/hooks/useAutoUpdate";
import { useSettingsStore } from "@/stores/settings";

function makeUpdate(version: string) {
  return { version, downloadAndInstall: vi.fn().mockResolvedValue(undefined) };
}

describe("useAutoUpdate", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    check.mockReset();
    ask.mockReset();
    relaunch.mockReset();
    useSettingsStore.setState({ autoUpdateIncludePrereleases: false });
  });

  afterEach(() => {
    vi.runOnlyPendingTimers();
    vi.useRealTimers();
  });

  it("does nothing until the 3s delay elapses", async () => {
    check.mockResolvedValue(null);
    renderHook(() => useAutoUpdate());
    expect(check).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(3000);
    expect(check).toHaveBeenCalledTimes(1);
  });

  it("clears the timer on unmount (no check fired)", async () => {
    check.mockResolvedValue(null);
    const { unmount } = renderHook(() => useAutoUpdate());
    unmount();
    await vi.advanceTimersByTimeAsync(3000);
    expect(check).not.toHaveBeenCalled();
  });

  it("returns early when there is no update", async () => {
    check.mockResolvedValue(null);
    renderHook(() => useAutoUpdate());
    await vi.advanceTimersByTimeAsync(3000);
    expect(ask).not.toHaveBeenCalled();
  });

  it("skips a pre-release update when the user has not opted in", async () => {
    check.mockResolvedValue(makeUpdate("1.2.0-rc.1"));
    renderHook(() => useAutoUpdate());
    await vi.advanceTimersByTimeAsync(3000);
    expect(ask).not.toHaveBeenCalled();
  });

  it("prompts for a stable update and does not relaunch when declined", async () => {
    check.mockResolvedValue(makeUpdate("1.2.0"));
    ask.mockResolvedValue(false);
    renderHook(() => useAutoUpdate());
    await vi.advanceTimersByTimeAsync(3000);
    expect(ask).toHaveBeenCalledWith(
      expect.stringContaining("1.2.0"),
      expect.objectContaining({ title: "Update Available" }),
    );
    expect(relaunch).not.toHaveBeenCalled();
  });

  it("installs and relaunches when a stable update is accepted", async () => {
    const update = makeUpdate("1.2.0");
    check.mockResolvedValue(update);
    ask.mockResolvedValue(true);
    renderHook(() => useAutoUpdate());
    await vi.advanceTimersByTimeAsync(3000);
    expect(update.downloadAndInstall).toHaveBeenCalledTimes(1);
    expect(relaunch).toHaveBeenCalledTimes(1);
  });

  it("prompts for a pre-release once the user opts in", async () => {
    useSettingsStore.setState({ autoUpdateIncludePrereleases: true });
    check.mockResolvedValue(makeUpdate("1.2.0-rc.1"));
    ask.mockResolvedValue(false);
    renderHook(() => useAutoUpdate());
    await vi.advanceTimersByTimeAsync(3000);
    expect(ask).toHaveBeenCalledTimes(1);
  });

  it("fails silently when the update check throws", async () => {
    check.mockRejectedValue(new Error("network down"));
    renderHook(() => useAutoUpdate());
    await expect(vi.advanceTimersByTimeAsync(3000)).resolves.not.toThrow();
    expect(ask).not.toHaveBeenCalled();
  });
});
