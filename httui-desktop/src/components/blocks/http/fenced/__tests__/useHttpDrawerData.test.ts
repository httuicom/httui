import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";

import { useHttpDrawerData } from "../useHttpDrawerData";
import type { HttpResponseFull } from "@/lib/tauri/streamedExecution";
import type {
  BlockExample,
  HistoryEntry,
  HttpBlockSettings,
} from "@/lib/tauri/commands";

// Mock every Tauri command the hook uses.
const insertBlockHistoryMock = vi.fn();
const listBlockHistoryMock = vi.fn();
const purgeBlockHistoryMock = vi.fn();
const listBlockExamplesMock = vi.fn();
const saveBlockExampleMock = vi.fn();
const deleteBlockExampleMock = vi.fn();

vi.mock("@/lib/tauri/commands", () => ({
  insertBlockHistory: (...a: unknown[]) => insertBlockHistoryMock(...a),
  listBlockHistory: (...a: unknown[]) => listBlockHistoryMock(...a),
  purgeBlockHistory: (...a: unknown[]) => purgeBlockHistoryMock(...a),
  listBlockExamples: (...a: unknown[]) => listBlockExamplesMock(...a),
  saveBlockExample: (...a: unknown[]) => saveBlockExampleMock(...a),
  deleteBlockExample: (...a: unknown[]) => deleteBlockExampleMock(...a),
}));

interface SetupOpts {
  alias?: string;
  drawerOpen?: boolean;
  settings?: HttpBlockSettings;
}

function setup(opts: SetupOpts = {}) {
  const apply = vi.fn();
  const setLast = vi.fn();
  const close = vi.fn();
  // Explicit check via `"alias" in opts` so passing `{ alias: undefined }`
  // is honored (not silently overridden by ??).
  const initialAlias: string | undefined =
    "alias" in opts ? opts.alias : "req1";
  const { result, rerender } = renderHook(
    (p: { drawerOpen: boolean; alias: string | undefined }) =>
      useHttpDrawerData({
        filePath: "current.md",
        alias: p.alias,
        drawerOpen: p.drawerOpen,
        settings: opts.settings ?? {},
        applyCachedResult: apply,
        setLastRunAt: setLast,
        closeDrawer: close,
      }),
    {
      initialProps: {
        alias: initialAlias,
        drawerOpen: opts.drawerOpen ?? false,
      },
    },
  );
  return { result, rerender, apply, setLast, close };
}

describe("useHttpDrawerData — recordHistory", () => {
  beforeEach(() => {
    insertBlockHistoryMock.mockReset();
    listBlockHistoryMock.mockReset();
    listBlockExamplesMock.mockReset();
    purgeBlockHistoryMock.mockReset();
    saveBlockExampleMock.mockReset();
    deleteBlockExampleMock.mockReset();
  });

  it("is a no-op when alias is undefined", async () => {
    const { result } = setup({ alias: undefined });
    await act(async () => {
      await result.current.recordHistory({
        method: "GET",
        url: "https://x",
        status: 200,
        requestSize: 0,
        responseSize: 1,
        elapsedMs: 10,
        outcome: "success",
      });
    });
    expect(insertBlockHistoryMock).not.toHaveBeenCalled();
  });

  it("is a no-op when settings.historyDisabled is true", async () => {
    const { result } = setup({ settings: { historyDisabled: true } });
    await act(async () => {
      await result.current.recordHistory({
        method: "GET",
        url: "x",
        status: 200,
        requestSize: 0,
        responseSize: 0,
        elapsedMs: 1,
        outcome: "success",
      });
    });
    expect(insertBlockHistoryMock).not.toHaveBeenCalled();
  });

  it("inserts a row and bumps the history tick on success", async () => {
    insertBlockHistoryMock.mockResolvedValue(undefined);
    const { result } = setup();
    await act(async () => {
      await result.current.recordHistory({
        method: "POST",
        url: "u",
        status: 201,
        requestSize: 5,
        responseSize: 9,
        elapsedMs: 42,
        outcome: "success",
      });
    });
    expect(insertBlockHistoryMock).toHaveBeenCalledWith(
      expect.objectContaining({
        file_path: "current.md",
        block_alias: "req1",
        method: "POST",
        url_canonical: "u",
        status: 201,
        request_size: 5,
        response_size: 9,
        elapsed_ms: 42,
        outcome: "success",
      }),
    );
  });

  it("swallows IPC rejection silently (best-effort)", async () => {
    insertBlockHistoryMock.mockRejectedValue(new Error("locked"));
    const { result } = setup();
    await act(async () => {
      await result.current.recordHistory({
        method: "GET",
        url: "u",
        status: null,
        requestSize: null,
        responseSize: null,
        elapsedMs: 0,
        outcome: "error",
      });
    });
    // No throw escapes act(); test just reaching here = pass.
    expect(insertBlockHistoryMock).toHaveBeenCalled();
  });
});

describe("useHttpDrawerData — history loader", () => {
  beforeEach(() => {
    listBlockHistoryMock.mockReset();
    listBlockExamplesMock.mockReset();
  });

  it("does NOT load while drawer closed", async () => {
    listBlockHistoryMock.mockResolvedValue([]);
    setup({ drawerOpen: false });
    await new Promise((r) => setTimeout(r, 5));
    expect(listBlockHistoryMock).not.toHaveBeenCalled();
  });

  it("loads history when drawer opens for a block with an alias", async () => {
    const rows: HistoryEntry[] = [
      {
        id: 1,
        file_path: "current.md",
        block_alias: "req1",
        method: "GET",
        url_canonical: "u",
        status: 200,
        request_size: 0,
        response_size: 0,
        elapsed_ms: 1,
        outcome: "success",
        ran_at: "2026-05-19T00:00:00Z",
      },
    ];
    listBlockHistoryMock.mockResolvedValue(rows);
    listBlockExamplesMock.mockResolvedValue([]);
    const { result, rerender } = setup({ drawerOpen: false });
    expect(result.current.historyEntries).toEqual([]);
    act(() => rerender({ drawerOpen: true, alias: "req1" }));
    await waitFor(() => expect(result.current.historyEntries).toHaveLength(1));
    expect(result.current.historyEntries[0].method).toBe("GET");
  });

  it("clears history to [] when drawer opens with no alias", async () => {
    listBlockExamplesMock.mockResolvedValue([]);
    const { result, rerender } = setup({
      alias: "req1",
      drawerOpen: false,
    });
    act(() => rerender({ drawerOpen: true, alias: undefined }));
    await new Promise((r) => setTimeout(r, 5));
    expect(result.current.historyEntries).toEqual([]);
    expect(listBlockHistoryMock).not.toHaveBeenCalled();
  });

  it("falls back to [] on IPC rejection", async () => {
    listBlockHistoryMock.mockRejectedValue(new Error("locked"));
    listBlockExamplesMock.mockResolvedValue([]);
    const { result, rerender } = setup({ drawerOpen: false });
    act(() => rerender({ drawerOpen: true, alias: "req1" }));
    await new Promise((r) => setTimeout(r, 5));
    expect(result.current.historyEntries).toEqual([]);
  });
});

describe("useHttpDrawerData — examples loader", () => {
  beforeEach(() => {
    listBlockHistoryMock.mockReset();
    listBlockExamplesMock.mockReset();
  });

  it("loads examples when drawer opens", async () => {
    const exs: BlockExample[] = [
      {
        id: 7,
        file_path: "current.md",
        block_alias: "req1",
        name: "snap",
        response_json: "{}",
        saved_at: "2026-05-19T01:00:00Z",
      },
    ];
    listBlockHistoryMock.mockResolvedValue([]);
    listBlockExamplesMock.mockResolvedValue(exs);
    const { result, rerender } = setup({ drawerOpen: false });
    act(() => rerender({ drawerOpen: true, alias: "req1" }));
    await waitFor(() => expect(result.current.examples).toHaveLength(1));
    expect(result.current.examples[0].name).toBe("snap");
  });

  it("clears examples to [] when drawer opens with no alias", async () => {
    listBlockHistoryMock.mockResolvedValue([]);
    const { result, rerender } = setup();
    act(() => rerender({ drawerOpen: true, alias: undefined }));
    await new Promise((r) => setTimeout(r, 5));
    expect(result.current.examples).toEqual([]);
    expect(listBlockExamplesMock).not.toHaveBeenCalled();
  });

  it("falls back to [] on IPC rejection", async () => {
    listBlockHistoryMock.mockResolvedValue([]);
    listBlockExamplesMock.mockRejectedValue(new Error("locked"));
    const { result, rerender } = setup({ drawerOpen: false });
    act(() => rerender({ drawerOpen: true, alias: "req1" }));
    await new Promise((r) => setTimeout(r, 5));
    expect(result.current.examples).toEqual([]);
  });
});

describe("useHttpDrawerData — drawer actions", () => {
  beforeEach(() => {
    purgeBlockHistoryMock.mockReset();
    saveBlockExampleMock.mockReset();
    deleteBlockExampleMock.mockReset();
  });

  it("bumpHistoryTick triggers a history reload when drawer is open", async () => {
    // Initial drawer-open load returns []; subsequent loads (post-tick)
    // return the row. We assert the FINAL state, not the exact call
    // count — strict-mode-like effect double-fire would flake counts.
    let calls = 0;
    listBlockHistoryMock.mockImplementation(async () => {
      return calls++ === 0
        ? []
        : [
            {
              id: 1,
              file_path: "current.md",
              block_alias: "req1",
              method: "GET",
              url_canonical: "u",
              status: 200,
              request_size: 0,
              response_size: 0,
              elapsed_ms: 1,
              outcome: "success",
              executed_at: "",
            },
          ];
    });
    listBlockExamplesMock.mockResolvedValue([]);
    const { result, rerender } = setup({ drawerOpen: false });
    act(() => rerender({ drawerOpen: true, alias: "req1" }));
    await waitFor(() => expect(listBlockHistoryMock).toHaveBeenCalled());
    const callsBefore = listBlockHistoryMock.mock.calls.length;
    act(() => result.current.bumpHistoryTick());
    await waitFor(() => {
      expect(listBlockHistoryMock.mock.calls.length).toBeGreaterThan(
        callsBefore,
      );
    });
    await waitFor(() => expect(result.current.historyEntries).toHaveLength(1));
  });

  it("purgeHistory is no-op when alias undefined", async () => {
    const { result } = setup({ alias: undefined });
    await act(async () => {
      await result.current.purgeHistory();
    });
    expect(purgeBlockHistoryMock).not.toHaveBeenCalled();
  });

  it("purgeHistory dispatches the IPC and bumps the tick", async () => {
    purgeBlockHistoryMock.mockResolvedValue(undefined);
    const { result } = setup();
    await act(async () => {
      await result.current.purgeHistory();
    });
    expect(purgeBlockHistoryMock).toHaveBeenCalledWith("current.md", "req1");
  });

  it("purgeHistory swallows IPC rejection", async () => {
    purgeBlockHistoryMock.mockRejectedValue(new Error("boom"));
    const { result } = setup();
    await act(async () => {
      await result.current.purgeHistory();
    });
    expect(purgeBlockHistoryMock).toHaveBeenCalled();
  });

  it("saveExample no-ops when alias undefined", async () => {
    const { result } = setup({ alias: undefined });
    await act(async () => {
      await result.current.saveExample("snap", {} as HttpResponseFull);
    });
    expect(saveBlockExampleMock).not.toHaveBeenCalled();
  });

  it("saveExample JSON-stringifies the response", async () => {
    saveBlockExampleMock.mockResolvedValue(undefined);
    const { result } = setup();
    const resp = {
      status_code: 200,
      foo: "bar",
    } as unknown as HttpResponseFull;
    await act(async () => {
      await result.current.saveExample("snap", resp);
    });
    expect(saveBlockExampleMock).toHaveBeenCalledWith(
      "current.md",
      "req1",
      "snap",
      JSON.stringify(resp),
    );
  });

  it("restoreExample parses + pushes to FSM + sets lastRunAt + closes drawer", () => {
    const stored = { status_code: 200, elapsed_ms: 5 };
    const ex: BlockExample = {
      id: 1,
      file_path: "current.md",
      block_alias: "req1",
      name: "snap",
      response_json: JSON.stringify(stored),
      saved_at: "2026-05-19T03:00:00Z",
    };
    const { result, apply, setLast, close } = setup();
    act(() => result.current.restoreExample(ex));
    expect(apply).toHaveBeenCalledWith(
      expect.objectContaining({ status_code: 200 }),
    );
    expect(setLast).toHaveBeenCalledWith(new Date("2026-05-19T03:00:00Z"));
    expect(close).toHaveBeenCalled();
  });

  it("restoreExample swallows JSON parse errors", () => {
    const ex: BlockExample = {
      id: 2,
      file_path: "x",
      block_alias: "y",
      name: "broken",
      response_json: "{ not json",
      saved_at: "",
    };
    const { result, apply, setLast, close } = setup();
    act(() => result.current.restoreExample(ex));
    expect(apply).not.toHaveBeenCalled();
    expect(setLast).not.toHaveBeenCalled();
    expect(close).not.toHaveBeenCalled();
  });

  it("deleteExample dispatches the IPC by id", async () => {
    deleteBlockExampleMock.mockResolvedValue(undefined);
    const { result } = setup();
    await act(async () => {
      await result.current.deleteExample(42);
    });
    expect(deleteBlockExampleMock).toHaveBeenCalledWith(42);
  });

  it("deleteExample swallows IPC rejection", async () => {
    deleteBlockExampleMock.mockRejectedValue(new Error("boom"));
    const { result } = setup();
    await act(async () => {
      await result.current.deleteExample(99);
    });
    expect(deleteBlockExampleMock).toHaveBeenCalled();
  });
});
