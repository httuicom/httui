import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";

import { useFileDocHeaderCompact } from "@/hooks/useFileDocHeaderCompact";

beforeEach(() => {
  clearTauriMocks();
});

afterEach(() => {
  clearTauriMocks();
});

describe("useFileDocHeaderCompact", () => {
  it("idles when vaultPath is null", async () => {
    const { result } = renderHook(() =>
      useFileDocHeaderCompact(null, "rollout.md"),
    );
    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current.compact).toBe(false);
    expect(result.current.loaded).toBe(false);
  });

  it("idles when filePath is null", async () => {
    const { result } = renderHook(() => useFileDocHeaderCompact("/v", null));
    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current.compact).toBe(false);
    expect(result.current.loaded).toBe(false);
  });

  it("populates docheader_compact after the initial poll", async () => {
    mockTauriCommand("get_file_settings", () => ({
      auto_capture: false,
      docheader_compact: true,
    }));

    const { result } = renderHook(() =>
      useFileDocHeaderCompact("/v", "rollout.md"),
    );
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(result.current.compact).toBe(true);
    expect(result.current.loaded).toBe(true);
  });

  it("treats undefined docheader_compact as false (skip-default wire shape)", async () => {
    // Rust serde skips default-valued booleans, so the field is
    // absent in JSON. The hook must coerce that to `false`, not
    // surface as truthy via `!== undefined`.
    mockTauriCommand("get_file_settings", () => ({ auto_capture: true }));

    const { result } = renderHook(() =>
      useFileDocHeaderCompact("/v", "rollout.md"),
    );
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(result.current.compact).toBe(false);
    expect(result.current.loaded).toBe(true);
  });

  it("falls back to false when the Tauri call errors", async () => {
    mockTauriCommand("get_file_settings", () => {
      throw new Error("io error");
    });

    const { result } = renderHook(() =>
      useFileDocHeaderCompact("/v", "rollout.md"),
    );
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(result.current.compact).toBe(false);
    expect(result.current.loaded).toBe(false);
  });

  it("setCompact calls through and updates state optimistically", async () => {
    let nextSettings: { auto_capture: boolean; docheader_compact?: boolean } = {
      auto_capture: false,
      docheader_compact: false,
    };
    mockTauriCommand("get_file_settings", () => nextSettings);
    const setCalls: { compact: boolean }[] = [];
    mockTauriCommand("set_file_docheader_compact", (args) => {
      const a = args as { compact: boolean };
      setCalls.push(a);
      nextSettings = {
        auto_capture: false,
        docheader_compact: Boolean(a.compact),
      };
      return null;
    });

    const { result } = renderHook(() =>
      useFileDocHeaderCompact("/v", "rollout.md"),
    );
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current.compact).toBe(false);

    await act(async () => {
      await result.current.setCompact(true);
    });

    expect(result.current.compact).toBe(true);
    expect(setCalls).toHaveLength(1);
    expect(setCalls[0]?.compact).toBe(true);
  });

  it("rolls back on persist failure", async () => {
    mockTauriCommand("get_file_settings", () => ({
      auto_capture: false,
      docheader_compact: false,
    }));
    mockTauriCommand("set_file_docheader_compact", () => {
      throw new Error("disk full");
    });

    const { result } = renderHook(() =>
      useFileDocHeaderCompact("/v", "rollout.md"),
    );
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current.compact).toBe(false);

    await act(async () => {
      try {
        await result.current.setCompact(true);
      } catch {
        // Expected — re-thrown so the caller can react.
      }
    });

    expect(result.current.compact).toBe(false);
  });

  it("setCompact is a no-op when paths are missing", async () => {
    let setCalls = 0;
    mockTauriCommand("set_file_docheader_compact", () => {
      setCalls += 1;
      return null;
    });

    const { result } = renderHook(() =>
      useFileDocHeaderCompact(null, "rollout.md"),
    );
    await act(async () => {
      await Promise.resolve();
    });
    await act(async () => {
      await result.current.setCompact(true);
    });
    expect(setCalls).toBe(0);
    expect(result.current.compact).toBe(false);
  });
});
