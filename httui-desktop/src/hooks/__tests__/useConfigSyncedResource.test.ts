import { describe, it, expect, afterEach, vi } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";

import {
  emitTauriEvent,
  clearTauriListeners,
  listen,
} from "@/test/mocks/tauri-event";

import { useConfigSyncedResource } from "@/hooks/useConfigSyncedResource";

afterEach(() => {
  clearTauriListeners();
  vi.clearAllMocks();
});

describe("useConfigSyncedResource", () => {
  it("refreshes once on mount", async () => {
    const refresh = vi.fn();
    renderHook(() => useConfigSyncedResource("connections", refresh));
    expect(refresh).toHaveBeenCalledTimes(1);
    // The config-changed subscription is wired (composed primitive).
    await waitFor(() =>
      expect(listen).toHaveBeenCalledWith(
        "config-changed",
        expect.any(Function),
      ),
    );
  });

  it("refreshes again when the matching category fires", async () => {
    const refresh = vi.fn();
    renderHook(() => useConfigSyncedResource("environment", refresh));
    await waitFor(() => expect(listen).toHaveBeenCalled());
    expect(refresh).toHaveBeenCalledTimes(1); // mount

    act(() => emitTauriEvent("config-changed", { category: "environment" }));
    expect(refresh).toHaveBeenCalledTimes(2); // + watcher
  });

  it("ignores a non-matching category", async () => {
    const refresh = vi.fn();
    renderHook(() => useConfigSyncedResource("environment", refresh));
    await waitFor(() => expect(listen).toHaveBeenCalled());
    refresh.mockClear();

    act(() => emitTauriEvent("config-changed", { category: "connections" }));
    expect(refresh).not.toHaveBeenCalled();
  });

  it("stops refreshing after unmount", async () => {
    const refresh = vi.fn();
    const { unmount } = renderHook(() =>
      useConfigSyncedResource("connections", refresh),
    );
    await waitFor(() => expect(listen).toHaveBeenCalled());
    refresh.mockClear();

    unmount();
    await waitFor(() => undefined);
    act(() => emitTauriEvent("config-changed", { category: "connections" }));
    expect(refresh).not.toHaveBeenCalled();
  });

  it("does not re-fire the mount refresh while the ref is stable", () => {
    const refresh = vi.fn();
    const { rerender } = renderHook(() =>
      useConfigSyncedResource("connections", refresh),
    );
    expect(refresh).toHaveBeenCalledTimes(1);
    rerender();
    rerender();
    expect(refresh).toHaveBeenCalledTimes(1);
  });
});
