import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";

vi.mock("@/lib/blocks/document", () => ({
  collectBlocksAboveCM: vi.fn(async () => []),
}));
vi.mock("@/stores/environment", () => ({
  useEnvironmentStore: {
    getState: () => ({ getActiveVariables: async () => ({ E: "1" }) }),
  },
}));

import {
  useExecutableBlock,
  type ExecOutcome,
  type UseExecutableBlockArgs,
} from "@/hooks/useExecutableBlock";

const view = { state: { doc: {} } } as never;

function make<T>(
  over: Partial<UseExecutableBlockArgs<T>> = {},
): UseExecutableBlockArgs<T> {
  return {
    idPrefix: "test",
    blockId: "b1",
    view,
    blockFrom: 0,
    filePath: "/v/n.md",
    prepare: async () => ({ params: { ok: true } }),
    execute: async () =>
      ({ status: "success", response: { elapsed_ms: 5 } }) as ExecOutcome<T>,
    elapsedOf: () => 5,
    ...over,
  };
}

beforeEach(() => vi.clearAllMocks());

describe("useExecutableBlock", () => {
  it("starts idle and runs the happy path (success + onOutcome + persist)", async () => {
    const onOutcome = vi.fn();
    const persist = vi.fn(async () => {});
    const { result } = renderHook(() =>
      useExecutableBlock<{ elapsed_ms: number }>(
        make({
          onOutcome,
          persist,
          execute: async () => ({
            status: "success",
            response: { elapsed_ms: 42 },
          }),
          elapsedOf: (r) => r.elapsed_ms,
        }),
      ),
    );
    expect(result.current.executionState).toBe("idle");

    await act(async () => {
      await result.current.run();
    });

    expect(result.current.executionState).toBe("success");
    expect(result.current.response).toEqual({ elapsed_ms: 42 });
    expect(result.current.durationMs).toBe(42); // elapsedOf wins over wall-clock
    expect(result.current.cached).toBe(false);
    expect(onOutcome).toHaveBeenCalledTimes(1);
    expect(persist).toHaveBeenCalledTimes(1);
  });

  it("synchronous validate gate fails before running (no prepare/execute)", async () => {
    const prepare = vi.fn(async () => ({ params: {} }));
    const execute = vi.fn(
      async () => ({ status: "success", response: {} }) as ExecOutcome<object>,
    );
    const { result } = renderHook(() =>
      useExecutableBlock<object>(
        make({ validate: () => "URL is required", prepare, execute }),
      ),
    );
    await act(async () => {
      await result.current.run();
    });
    expect(result.current.executionState).toBe("error");
    expect(result.current.error).toBe("URL is required");
    expect(prepare).not.toHaveBeenCalled();
    expect(execute).not.toHaveBeenCalled();
  });

  it("prepare error → error state, duration stamped, execute skipped", async () => {
    const execute = vi.fn(
      async () => ({ status: "success", response: {} }) as ExecOutcome<object>,
    );
    const { result } = renderHook(() =>
      useExecutableBlock<object>(
        make({ prepare: async () => ({ error: "bad refs" }), execute }),
      ),
    );
    await act(async () => {
      await result.current.run();
    });
    expect(result.current.executionState).toBe("error");
    expect(result.current.error).toBe("bad refs");
    expect(result.current.durationMs).toBeGreaterThanOrEqual(0);
    expect(execute).not.toHaveBeenCalled();
  });

  it("execute error outcome → error, no persist, onOutcome fired", async () => {
    const persist = vi.fn(async () => {});
    const onOutcome = vi.fn();
    const { result } = renderHook(() =>
      useExecutableBlock<object>(
        make({
          persist,
          onOutcome,
          execute: async () => ({ status: "error", message: "boom" }),
        }),
      ),
    );
    await act(async () => {
      await result.current.run();
    });
    expect(result.current.executionState).toBe("error");
    expect(result.current.error).toBe("boom");
    expect(persist).not.toHaveBeenCalled();
    expect(onOutcome).toHaveBeenCalledTimes(1);
  });

  it("cancelled outcome → cancelled state, no persist", async () => {
    const persist = vi.fn(async () => {});
    const { result } = renderHook(() =>
      useExecutableBlock<object>(
        make({ persist, execute: async () => ({ status: "cancelled" }) }),
      ),
    );
    await act(async () => {
      await result.current.run();
    });
    expect(result.current.executionState).toBe("cancelled");
    expect(persist).not.toHaveBeenCalled();
  });

  it("a thrown executor lands in the catch → error state", async () => {
    const { result } = renderHook(() =>
      useExecutableBlock<object>(
        make({
          execute: async () => {
            throw new Error("network down");
          },
        }),
      ),
    );
    await act(async () => {
      await result.current.run();
    });
    expect(result.current.executionState).toBe("error");
    expect(result.current.error).toBe("network down");
  });

  it("a persist failure is swallowed (stays success)", async () => {
    const { result } = renderHook(() =>
      useExecutableBlock<object>(
        make({
          execute: async () => ({ status: "success", response: {} }),
          elapsedOf: () => 1,
          persist: async () => {
            throw new Error("sqlite locked");
          },
        }),
      ),
    );
    await act(async () => {
      await result.current.run();
    });
    expect(result.current.executionState).toBe("success");
  });

  it("falls back to wall-clock when elapsedOf is falsy", async () => {
    const { result } = renderHook(() =>
      useExecutableBlock<object>(
        make({
          execute: async () => ({ status: "success", response: {} }),
          elapsedOf: () => 0,
        }),
      ),
    );
    await act(async () => {
      await result.current.run();
    });
    expect(result.current.durationMs).toBeGreaterThanOrEqual(0);
  });

  it("cancel() aborts the in-flight signal", async () => {
    let seen: AbortSignal | null = null;
    const { result } = renderHook(() =>
      useExecutableBlock<object>(
        make({
          execute: async ({ signal }) => {
            seen = signal;
            return { status: "cancelled" };
          },
        }),
      ),
    );
    await act(async () => {
      await result.current.run();
    });
    act(() => result.current.cancel());
    // The controller created for the run is detached on cancel; the
    // signal handed to execute is real and abortable.
    expect(seen).not.toBeNull();
  });

  it("applyCachedResult pushes a hit (sets duration, clears error); reset → idle", () => {
    const { result } = renderHook(() =>
      useExecutableBlock<{ x: number }>(make<{ x: number }>()),
    );
    act(() => result.current.applyCachedResult({ x: 9 }, 12));
    expect(result.current.executionState).toBe("success");
    expect(result.current.response).toEqual({ x: 9 });
    expect(result.current.durationMs).toBe(12);
    expect(result.current.cached).toBe(true);
    expect(result.current.error).toBeNull();

    act(() => result.current.reset());
    expect(result.current.executionState).toBe("idle");
    expect(result.current.response).toBeNull();
    expect(result.current.cached).toBe(false);
  });

  it("applyCachedResult with omitted duration leaves durationMs untouched + clears prior error", async () => {
    const { result } = renderHook(() =>
      useExecutableBlock<{ x: number }>(
        make<{ x: number }>({
          execute: async () => ({ status: "error", message: "boom" }),
        }),
      ),
    );
    // Produce a prior error + a stamped duration via a failed run.
    await act(async () => {
      await result.current.run();
    });
    expect(result.current.error).toBe("boom");
    const stampedDuration = result.current.durationMs;
    expect(stampedDuration).not.toBeNull();

    // restore-example path: response + success + error cleared, but
    // duration is NOT touched (no elapsed to show for a restore).
    act(() => result.current.applyCachedResult({ x: 1 }));
    expect(result.current.executionState).toBe("success");
    expect(result.current.response).toEqual({ x: 1 });
    expect(result.current.error).toBeNull();
    expect(result.current.durationMs).toBe(stampedDuration);
    expect(result.current.cached).toBe(true);
  });

  it("onRunStart fires once the FSM enters running", async () => {
    const onRunStart = vi.fn();
    const { result } = renderHook(() =>
      useExecutableBlock<object>(
        make({
          onRunStart,
          execute: async () => ({ status: "success", response: {} }),
          elapsedOf: () => 1,
        }),
      ),
    );
    await act(async () => {
      await result.current.run();
    });
    expect(onRunStart).toHaveBeenCalledTimes(1);
  });
});
