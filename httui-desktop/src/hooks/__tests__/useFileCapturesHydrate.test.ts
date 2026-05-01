import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";
import { useCaptureStore } from "@/stores/captureStore";
import { useFileCapturesHydrate } from "@/hooks/useFileCapturesHydrate";

const VAULT = "/v";
const FILE = "rb.md";

beforeEach(() => {
  clearTauriMocks();
  useCaptureStore.setState({ values: {} }, false);
});

afterEach(() => clearTauriMocks());

describe("useFileCapturesHydrate", () => {
  it("hydrates the captureStore when the cache returns JSON", async () => {
    const json = JSON.stringify({
      loginBlock: { session_id: "s_123" },
    });
    mockTauriCommand("read_captures_cache_cmd", () => json);

    renderHook(() => useFileCapturesHydrate(VAULT, FILE));

    await waitFor(() => {
      const entry = useCaptureStore
        .getState()
        .getCapture(FILE, "loginBlock", "session_id");
      expect(entry?.value).toBe("s_123");
    });
  });

  it("is a no-op when the cache returns null (no persisted file)", async () => {
    const reader = vi.fn(() => null);
    mockTauriCommand("read_captures_cache_cmd", reader);

    renderHook(() => useFileCapturesHydrate(VAULT, FILE));

    await waitFor(() => expect(reader).toHaveBeenCalledTimes(1));
    // Nothing got hydrated.
    expect(useCaptureStore.getState().values).toEqual({});
  });

  it("swallows reader errors so the editor doesn't surface them", async () => {
    mockTauriCommand("read_captures_cache_cmd", () => {
      throw new Error("ENOENT or perms or whatever");
    });

    // Should not throw.
    renderHook(() => useFileCapturesHydrate(VAULT, FILE));

    // Give the async path a tick.
    await new Promise((r) => setTimeout(r, 0));
    expect(useCaptureStore.getState().values).toEqual({});
  });

  it("idles when vaultPath is null", async () => {
    const reader = vi.fn();
    mockTauriCommand("read_captures_cache_cmd", reader);

    renderHook(() => useFileCapturesHydrate(null, FILE));

    await new Promise((r) => setTimeout(r, 0));
    expect(reader).not.toHaveBeenCalled();
  });

  it("idles when filePath is null", async () => {
    const reader = vi.fn();
    mockTauriCommand("read_captures_cache_cmd", reader);

    renderHook(() => useFileCapturesHydrate(VAULT, null));

    await new Promise((r) => setTimeout(r, 0));
    expect(reader).not.toHaveBeenCalled();
  });

  it("re-fetches when filePath changes", async () => {
    const reader = vi.fn((args: unknown) => {
      const fp = (args as { filePath: string }).filePath;
      if (fp === "a.md") {
        return JSON.stringify({ b1: { token: "from-a" } });
      }
      if (fp === "b.md") {
        return JSON.stringify({ b1: { token: "from-b" } });
      }
      return null;
    });
    mockTauriCommand("read_captures_cache_cmd", reader);

    const { rerender } = renderHook(
      ({ file }: { file: string }) => useFileCapturesHydrate(VAULT, file),
      { initialProps: { file: "a.md" } },
    );
    await waitFor(() => {
      expect(
        useCaptureStore.getState().getCapture("a.md", "b1", "token")?.value,
      ).toBe("from-a");
    });

    rerender({ file: "b.md" });
    await waitFor(() => {
      expect(
        useCaptureStore.getState().getCapture("b.md", "b1", "token")?.value,
      ).toBe("from-b");
    });
    expect(reader).toHaveBeenCalledTimes(2);
  });
});
