import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";

import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";
import { useCaptureStore } from "@/stores/captureStore";
import { useFileCapturesPersistence } from "@/hooks/useFileCapturesPersistence";

// `useFileAutoCapture` reads `get_file_settings` and writes via
// `set_file_auto_capture`. We exercise the real inner hook with
// Tauri-IPC mocks so the wrapper's compose contract is tested
// end-to-end (no internal mock of useFileAutoCapture).

const VAULT = "/v";
const FILE = "runbooks/x.md";

function seedAutoCaptureFlag(initial: boolean) {
  let current = initial;
  mockTauriCommand("get_file_settings", () => ({
    auto_capture: current,
    docheader_compact: false,
  }));
  mockTauriCommand("set_file_auto_capture", (args: unknown) => {
    const next = (args as { autoCapture?: boolean }).autoCapture ?? false;
    current = next;
  });
}

beforeEach(() => {
  clearTauriMocks();
  // Reset the captures store between cases so dumps are deterministic.
  useCaptureStore.setState({ values: {} }, false);
});

afterEach(() => {
  clearTauriMocks();
});

describe("useFileCapturesPersistence", () => {
  it("writes the cache file when toggling auto-capture ON with captures present", async () => {
    seedAutoCaptureFlag(false);
    const writes = vi.fn();
    mockTauriCommand("write_captures_cache_cmd", (args) => {
      writes(args);
      return "ok";
    });

    // Seed a public capture for the file.
    useCaptureStore.getState().setBlockCaptures(FILE, "loginBlock", {
      session_id: "s_abc",
    });

    const { result } = renderHook(() =>
      useFileCapturesPersistence(VAULT, FILE),
    );
    await waitFor(() => expect(result.current.loaded).toBe(true));

    await act(async () => {
      await result.current.setAutoCapture(true);
    });

    expect(writes).toHaveBeenCalledTimes(1);
    const arg = writes.mock.calls[0]![0] as {
      vaultPath: string;
      filePath: string;
      json: string;
    };
    expect(arg.vaultPath).toBe(VAULT);
    expect(arg.filePath).toBe(FILE);
    const parsed = JSON.parse(arg.json) as Record<string, unknown>;
    expect(parsed["loginBlock"]).toMatchObject({
      session_id: "s_abc",
    });
  });

  it("skips the cache write when there's nothing to persist", async () => {
    seedAutoCaptureFlag(false);
    const writes = vi.fn();
    mockTauriCommand("write_captures_cache_cmd", (args) => {
      writes(args);
      return "ok";
    });

    const { result } = renderHook(() =>
      useFileCapturesPersistence(VAULT, FILE),
    );
    await waitFor(() => expect(result.current.loaded).toBe(true));

    await act(async () => {
      await result.current.setAutoCapture(true);
    });

    expect(writes).not.toHaveBeenCalled();
  });

  it("deletes the cache file when toggling auto-capture OFF", async () => {
    seedAutoCaptureFlag(true);
    const deletes = vi.fn();
    mockTauriCommand("delete_captures_cache_cmd", (args) => {
      deletes(args);
      return true;
    });

    const { result } = renderHook(() =>
      useFileCapturesPersistence(VAULT, FILE),
    );
    await waitFor(() => expect(result.current.loaded).toBe(true));

    await act(async () => {
      await result.current.setAutoCapture(false);
    });

    expect(deletes).toHaveBeenCalledTimes(1);
    expect(deletes.mock.calls[0]![0]).toMatchObject({
      vaultPath: VAULT,
      filePath: FILE,
    });
  });

  it("swallows cache I/O errors so the UI doesn't lie about the toggle", async () => {
    seedAutoCaptureFlag(true);
    mockTauriCommand("delete_captures_cache_cmd", () => {
      throw new Error("ENOENT");
    });

    const { result } = renderHook(() =>
      useFileCapturesPersistence(VAULT, FILE),
    );
    await waitFor(() => expect(result.current.loaded).toBe(true));

    // Should not throw — cache layer is best-effort.
    await act(async () => {
      await result.current.setAutoCapture(false);
    });

    // Inner persist still ran successfully → flag flipped.
    expect(result.current.autoCapture).toBe(false);
  });

  it("skips cache I/O when the inner persist throws (rollback path)", async () => {
    seedAutoCaptureFlag(false);
    mockTauriCommand("set_file_auto_capture", () => {
      throw new Error("disk full");
    });
    const writes = vi.fn();
    mockTauriCommand("write_captures_cache_cmd", (args) => {
      writes(args);
      return "ok";
    });

    const { result } = renderHook(() =>
      useFileCapturesPersistence(VAULT, FILE),
    );
    await waitFor(() => expect(result.current.loaded).toBe(true));

    await act(async () => {
      await expect(result.current.setAutoCapture(true)).rejects.toThrow(
        /disk full/,
      );
    });

    expect(writes).not.toHaveBeenCalled();
    // Optimistic update rolled back.
    expect(result.current.autoCapture).toBe(false);
  });

  it("is a no-op when vaultPath or filePath is null", async () => {
    const writes = vi.fn();
    const deletes = vi.fn();
    mockTauriCommand("write_captures_cache_cmd", (a) => {
      writes(a);
      return "ok";
    });
    mockTauriCommand("delete_captures_cache_cmd", (a) => {
      deletes(a);
      return true;
    });

    const { result } = renderHook(() => useFileCapturesPersistence(null, FILE));
    // Inner is idle when vault is null — no Tauri calls fired.
    await act(async () => {
      await result.current.setAutoCapture(true);
    });
    expect(writes).not.toHaveBeenCalled();
    expect(deletes).not.toHaveBeenCalled();
  });
});
