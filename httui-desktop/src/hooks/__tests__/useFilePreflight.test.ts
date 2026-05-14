import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";

import { useFilePreflight } from "@/hooks/useFilePreflight";
import { clearTauriMocks, mockTauriCommand } from "@/test/mocks/tauri";
import { usePaneStore } from "@/stores/pane";
import type { EvaluatedPreflightItem } from "@/lib/tauri/preflight";

describe("useFilePreflight", () => {
  beforeEach(() => {
    clearTauriMocks();
    // Reset saveSignal so cross-test bumps don't leak.
    usePaneStore.setState({ saveSignal: 0 });
  });

  afterEach(() => {
    clearTauriMocks();
  });

  it("returns empty items + non-rechecking when vaultPath is null", async () => {
    let called = false;
    mockTauriCommand("evaluate_preflight_cmd", () => {
      called = true;
      return [];
    });

    const { result } = renderHook(() =>
      useFilePreflight({ filePath: "/v/x.md", vaultPath: null }),
    );
    await act(async () => {
      await Promise.resolve();
    });
    expect(called).toBe(false);
    expect(result.current.items).toEqual([]);
    expect(result.current.rechecking).toBe(false);
  });

  it("fetches and maps the response to PreflightPillItem[] on mount", async () => {
    const raw: EvaluatedPreflightItem[] = [
      {
        kind: "file_exists",
        label: "schema.sql",
        result: { outcome: "pass" },
      },
      {
        kind: "command",
        label: "psql",
        result: { outcome: "fail", reason: "command `psql` not found in PATH" },
      },
    ];
    mockTauriCommand("evaluate_preflight_cmd", () => raw);

    const { result } = renderHook(() =>
      useFilePreflight({
        filePath: "/v/note.md",
        vaultPath: "/v",
      }),
    );
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(result.current.items).toHaveLength(2);
    expect(result.current.items[0]?.label).toBe("schema.sql");
    expect(result.current.items[0]?.result).toEqual({ outcome: "pass" });
    expect(result.current.items[1]?.label).toBe("psql");
    expect(result.current.items[1]?.suggestion).toMatch(/psql/);
  });

  it("re-fetches when pane store's saveSignal bumps (auto-save resolved)", async () => {
    let calls = 0;
    mockTauriCommand("evaluate_preflight_cmd", () => {
      calls++;
      return [];
    });

    renderHook(() =>
      useFilePreflight({
        filePath: "/v/note.md",
        vaultPath: "/v",
      }),
    );
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    const callsAfterMount = calls;

    await act(async () => {
      usePaneStore.getState().notifySaved();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(calls).toBe(callsAfterMount + 1);
  });

  it("does not re-fetch when saveSignal stays the same", async () => {
    let calls = 0;
    mockTauriCommand("evaluate_preflight_cmd", () => {
      calls++;
      return [];
    });

    const { rerender } = renderHook(() =>
      useFilePreflight({
        filePath: "/v/note.md",
        vaultPath: "/v",
      }),
    );
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    const callsAfterMount = calls;

    rerender();
    await act(async () => {
      await Promise.resolve();
    });
    expect(calls).toBe(callsAfterMount);
  });

  it("recheck() triggers a fresh fetch", async () => {
    let calls = 0;
    mockTauriCommand("evaluate_preflight_cmd", () => {
      calls++;
      return [];
    });

    const { result } = renderHook(() =>
      useFilePreflight({
        filePath: "/v/note.md",
        vaultPath: "/v",
      }),
    );
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    const callsAfterMount = calls;

    await act(async () => {
      result.current.recheck();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(calls).toBe(callsAfterMount + 1);
  });

  it("swallows backend errors and clears items", async () => {
    mockTauriCommand("evaluate_preflight_cmd", () => {
      throw new Error("rpc fail");
    });

    const { result } = renderHook(() =>
      useFilePreflight({
        filePath: "/v/note.md",
        vaultPath: "/v",
      }),
    );
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current.items).toEqual([]);
  });

  it("includes a suggestion for each known kind", async () => {
    const raw: EvaluatedPreflightItem[] = [
      { kind: "connection", label: "payments-db", result: { outcome: "pass" } },
      { kind: "env_var", label: "API_TOKEN", result: { outcome: "pass" } },
      { kind: "branch", label: "main", result: { outcome: "pass" } },
      { kind: "unknown", label: "future", result: { outcome: "skip", reason: "x" } },
    ];
    mockTauriCommand("evaluate_preflight_cmd", () => raw);

    const { result } = renderHook(() =>
      useFilePreflight({
        filePath: "/v/note.md",
        vaultPath: "/v",
      }),
    );
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(result.current.items[0]?.suggestion).toMatch(/payments-db/);
    expect(result.current.items[1]?.suggestion).toMatch(/API_TOKEN/);
    expect(result.current.items[2]?.suggestion).toMatch(/main/);
    // Unknown kind: no suggestion (the future kind isn't actionable
    // until the parser learns it).
    expect(result.current.items[3]?.suggestion).toBeUndefined();
  });
});
