import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

import { useHttpRefsContext } from "../useHttpRefsContext";

// Mock the heavy deps so the hook can run in isolation.
const collectBlocksAboveCMMock = vi.fn();
vi.mock("@/lib/blocks/document", () => ({
  collectBlocksAboveCM: (...args: unknown[]) =>
    collectBlocksAboveCMMock(...args),
}));

const getActiveVariablesMock = vi.fn();
vi.mock("@/stores/environment", () => ({
  useEnvironmentStore: {
    getState: () => ({
      getActiveVariables: () => getActiveVariablesMock(),
    }),
  },
}));

// Minimal `EditorView` shim — the hook only reads `view.state.doc`,
// and only as a useEffect dep key (the value is passed to
// `collectBlocksAboveCM` which is mocked above).
const fakeView = (docId: string) =>
  ({ state: { doc: { id: docId } } }) as unknown as Parameters<
    typeof useHttpRefsContext
  >[0];

describe("useHttpRefsContext", () => {
  beforeEach(() => {
    collectBlocksAboveCMMock.mockReset();
    getActiveVariablesMock.mockReset();
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("populates blocks + envKeys from the async sources", async () => {
    collectBlocksAboveCMMock.mockResolvedValue([
      { alias: "a", response: { status: 200 } },
    ]);
    getActiveVariablesMock.mockResolvedValue({ FOO: "bar", BAZ: "qux" });

    const { result } = renderHook(() =>
      useHttpRefsContext(fakeView("doc-1"), 0, "current.md"),
    );

    // Getters are immediately defined (they read a ref); the load is async.
    expect(typeof result.current.getBlocks).toBe("function");
    expect(typeof result.current.getEnvKeys).toBe("function");

    await waitFor(() => {
      expect(result.current.getBlocks()).toHaveLength(1);
    });
    expect(result.current.getBlocks()[0]).toMatchObject({ alias: "a" });
    expect(result.current.getEnvKeys().sort()).toEqual(["BAZ", "FOO"]);
  });

  it("starts with empty blocks + envKeys before the load resolves", () => {
    collectBlocksAboveCMMock.mockReturnValue(new Promise(() => {}));
    getActiveVariablesMock.mockReturnValue(new Promise(() => {}));

    const { result } = renderHook(() =>
      useHttpRefsContext(fakeView("doc-x"), 0, "x.md"),
    );

    expect(result.current.getBlocks()).toEqual([]);
    expect(result.current.getEnvKeys()).toEqual([]);
  });

  it("swallows errors from the load (best-effort) and keeps prior state", async () => {
    // 1st render — succeeds.
    collectBlocksAboveCMMock.mockResolvedValueOnce([
      { alias: "first", response: {} },
    ]);
    getActiveVariablesMock.mockResolvedValueOnce({ A: "1" });

    const { result, rerender } = renderHook(
      ({ from }) => useHttpRefsContext(fakeView("doc-1"), from, "x.md"),
      { initialProps: { from: 0 } },
    );

    await waitFor(() => {
      expect(result.current.getBlocks()).toHaveLength(1);
    });

    // 2nd render with new `from` — throws. State should NOT regress.
    collectBlocksAboveCMMock.mockRejectedValueOnce(new Error("boom"));
    getActiveVariablesMock.mockResolvedValueOnce({ A: "1" });
    rerender({ from: 10 });

    // Allow the failed load to settle.
    await new Promise((r) => setTimeout(r, 5));
    expect(result.current.getBlocks()).toHaveLength(1);
    expect(result.current.getBlocks()[0]).toMatchObject({ alias: "first" });
  });

  it("returns stable getter identities across re-renders", () => {
    collectBlocksAboveCMMock.mockResolvedValue([]);
    getActiveVariablesMock.mockResolvedValue({});

    const { result, rerender } = renderHook(
      ({ from }) => useHttpRefsContext(fakeView("doc-1"), from, "x.md"),
      { initialProps: { from: 0 } },
    );
    const firstGetters = result.current;
    rerender({ from: 5 });
    expect(result.current.getBlocks).toBe(firstGetters.getBlocks);
    expect(result.current.getEnvKeys).toBe(firstGetters.getEnvKeys);
  });

  it("cancels a pending load when the dep changes mid-flight", async () => {
    // 1st render — returns a never-settling promise we can resolve manually.
    let resolveFirst!: (rows: unknown[]) => void;
    collectBlocksAboveCMMock.mockReturnValueOnce(
      new Promise((res) => {
        resolveFirst = res as (rows: unknown[]) => void;
      }),
    );
    getActiveVariablesMock.mockResolvedValueOnce({});

    const { result, rerender } = renderHook(
      ({ from }) => useHttpRefsContext(fakeView("doc-1"), from, "x.md"),
      { initialProps: { from: 0 } },
    );

    // 2nd render before the 1st promise settles — fresh dep keys cancel it.
    collectBlocksAboveCMMock.mockResolvedValueOnce([
      { alias: "second", response: {} },
    ]);
    getActiveVariablesMock.mockResolvedValueOnce({});
    rerender({ from: 99 });

    // Resolve the stale 1st promise AFTER the cancel; its values must not
    // be written into the ref.
    resolveFirst([{ alias: "stale", response: {} }]);

    await waitFor(() => {
      expect(result.current.getBlocks().some((b) => b.alias === "second")).toBe(
        true,
      );
    });
    expect(result.current.getBlocks().some((b) => b.alias === "stale")).toBe(
      false,
    );
  });
});
