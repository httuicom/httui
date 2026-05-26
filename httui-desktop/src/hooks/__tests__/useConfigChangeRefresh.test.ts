import { describe, it, expect, afterEach, vi } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";
import {
  emitTauriEvent,
  clearTauriListeners,
  listen,
} from "@/test/mocks/tauri-event";

import { useConfigChangeRefresh } from "@/hooks/useConfigChangeRefresh";

afterEach(() => {
  clearTauriListeners();
  vi.clearAllMocks();
});

describe("useConfigChangeRefresh", () => {
  it("invokes onChange when the matching category fires", async () => {
    const onChange = vi.fn();
    renderHook(() => useConfigChangeRefresh("connections", onChange));
    await waitFor(() =>
      expect(listen).toHaveBeenCalledWith(
        "config-changed",
        expect.any(Function),
      ),
    );

    act(() => emitTauriEvent("config-changed", { category: "connections" }));
    expect(onChange).toHaveBeenCalledTimes(1);
  });

  it("ignores a non-matching category", async () => {
    const onChange = vi.fn();
    renderHook(() => useConfigChangeRefresh("environment", onChange));
    await waitFor(() => expect(listen).toHaveBeenCalled());

    act(() => emitTauriEvent("config-changed", { category: "connections" }));
    expect(onChange).not.toHaveBeenCalled();
  });

  it("unsubscribes on unmount", async () => {
    const onChange = vi.fn();
    const { unmount } = renderHook(() =>
      useConfigChangeRefresh("connections", onChange),
    );
    await waitFor(() => expect(listen).toHaveBeenCalled());

    unmount();
    // Let the async-listen cleanup settle, then the (now-detached)
    // subscription must not deliver.
    await waitFor(() => undefined);
    act(() => emitTauriEvent("config-changed", { category: "connections" }));
    expect(onChange).not.toHaveBeenCalled();
  });
});
