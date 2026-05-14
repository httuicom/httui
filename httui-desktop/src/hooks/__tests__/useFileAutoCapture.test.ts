import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";

import { useFileAutoCapture } from "@/hooks/useFileAutoCapture";

beforeEach(() => {
  clearTauriMocks();
});

afterEach(() => {
  clearTauriMocks();
});

describe("useFileAutoCapture", () => {
  it("idles when vaultPath is null", async () => {
    const { result } = renderHook(() =>
      useFileAutoCapture(null, "rollout.md"),
    );
    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current.autoCapture).toBe(false);
    expect(result.current.loaded).toBe(false);
  });

  it("idles when filePath is null", async () => {
    const { result } = renderHook(() => useFileAutoCapture("/v", null));
    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current.autoCapture).toBe(false);
    expect(result.current.loaded).toBe(false);
  });

  it("populates auto_capture after the initial poll", async () => {
    mockTauriCommand("get_file_settings", () => ({ auto_capture: true }));

    const { result } = renderHook(() =>
      useFileAutoCapture("/v", "rollout.md"),
    );
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(result.current.autoCapture).toBe(true);
    expect(result.current.loaded).toBe(true);
  });

  it("falls back to false when the Tauri call errors", async () => {
    mockTauriCommand("get_file_settings", () => {
      throw new Error("io error");
    });

    const { result } = renderHook(() =>
      useFileAutoCapture("/v", "rollout.md"),
    );
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(result.current.autoCapture).toBe(false);
    expect(result.current.loaded).toBe(false);
  });

  it("setAutoCapture calls through and updates state optimistically", async () => {
    let nextSettings = { auto_capture: false };
    mockTauriCommand("get_file_settings", () => nextSettings);
    const setCalls: { autoCapture: boolean }[] = [];
    mockTauriCommand("set_file_auto_capture", (args) => {
      setCalls.push(args as { autoCapture: boolean });
      nextSettings = { auto_capture: Boolean((args as { autoCapture: boolean }).autoCapture) };
      return null;
    });

    const { result } = renderHook(() =>
      useFileAutoCapture("/v", "rollout.md"),
    );
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current.autoCapture).toBe(false);

    await act(async () => {
      await result.current.setAutoCapture(true);
    });

    expect(result.current.autoCapture).toBe(true);
    expect(setCalls).toHaveLength(1);
    expect(setCalls[0]?.autoCapture).toBe(true);
  });

  it("rolls back on persist failure", async () => {
    mockTauriCommand("get_file_settings", () => ({ auto_capture: false }));
    mockTauriCommand("set_file_auto_capture", () => {
      throw new Error("disk full");
    });

    const { result } = renderHook(() =>
      useFileAutoCapture("/v", "rollout.md"),
    );
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current.autoCapture).toBe(false);

    await act(async () => {
      try {
        await result.current.setAutoCapture(true);
      } catch {
        // Expected — the hook re-throws so the caller can react.
      }
    });

    // Optimistic update rolled back to the previous value.
    expect(result.current.autoCapture).toBe(false);
  });

  it("setAutoCapture is a no-op when paths are missing", async () => {
    let setCalls = 0;
    mockTauriCommand("set_file_auto_capture", () => {
      setCalls += 1;
      return null;
    });

    const { result } = renderHook(() =>
      useFileAutoCapture(null, "rollout.md"),
    );
    await act(async () => {
      await Promise.resolve();
    });
    await act(async () => {
      await result.current.setAutoCapture(true);
    });
    expect(setCalls).toBe(0);
    expect(result.current.autoCapture).toBe(false);
  });
});
