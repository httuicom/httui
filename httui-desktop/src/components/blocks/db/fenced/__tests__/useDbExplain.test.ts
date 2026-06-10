// EXPLAIN runner: first statement only, dialect-aware prefix, plan rows
// folded into the existing response, SQL errors surfaced as panel
// errors — never as Plan-tab noise.
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderHook } from "@testing-library/react";

const executeDbStreamed = vi.fn();
vi.mock("@/lib/tauri/streamedExecution", () => ({
  executeDbStreamed: (...args: unknown[]) => executeDbStreamed(...args),
}));
vi.mock("@/lib/blocks/document", () => ({
  collectBlocksAboveCM: vi.fn(async () => []),
}));
vi.mock("@/stores/environment", () => ({
  useEnvironmentStore: {
    getState: () => ({ getActiveVariables: async () => ({}) }),
  },
}));

import { useDbExplain, type DbRunStateSetters } from "../useDbExplain";
import type { Connection } from "@/lib/tauri/connections";
import type { DbResponse } from "@/components/blocks/db/types";
import type { ExecutionState } from "../shared";
import {
  makeConnection,
  makeDbBlock,
  makeView,
  selectResponse,
} from "./helpers";

beforeEach(() => {
  executeDbStreamed.mockReset();
});

function setup(opts: {
  body?: string;
  connection?: Connection | null;
  executionState?: ExecutionState;
  response?: DbResponse | null;
}) {
  const view = makeView("");
  const block = makeDbBlock(view, { alias: "q1" }, opts.body ?? "SELECT 1;");
  let response: DbResponse | null = opts.response ?? null;
  const states: ExecutionState[] = [];
  const errors: (string | null)[] = [];
  const setters: DbRunStateSetters = {
    setExecutionState: (s) => states.push(s),
    setError: (e) => errors.push(e),
    setDurationMs: vi.fn(),
    setResponse: (updater) => {
      response = updater(response);
    },
    setCached: vi.fn(),
  };
  const abortRef = { current: null as AbortController | null };
  const { result } = renderHook(() =>
    useDbExplain({
      executionState: opts.executionState ?? "idle",
      activeConnection:
        opts.connection === undefined ? makeConnection() : opts.connection,
      block,
      blockId: "db_idx_0",
      view,
      filePath: "f.md",
      abortRef,
      setters,
    }),
  );
  return {
    runExplain: result.current,
    states,
    errors,
    getResponse: () => response,
  };
}

describe("useDbExplain", () => {
  it("wraps the first statement and folds plan rows into the response", async () => {
    executeDbStreamed.mockResolvedValue({
      status: "success",
      response: selectResponse([{ plan: "Seq Scan" }]),
    });
    const { runExplain, states, getResponse } = setup({
      body: "SELECT 1;\nSELECT 2;",
    });

    await runExplain();

    const call = executeDbStreamed.mock.calls[0][0];
    expect(call.params.query).toBe("EXPLAIN SELECT 1");
    expect(getResponse()?.plan).toEqual([{ plan: "Seq Scan" }]);
    expect(states.at(-1)).toBe("success");
  });

  it("uses EXPLAIN QUERY PLAN for sqlite drivers", async () => {
    executeDbStreamed.mockResolvedValue({
      status: "success",
      response: selectResponse([{ detail: "SCAN t" }]),
    });
    const { runExplain } = setup({
      connection: makeConnection({ driver: "sqlite" }),
    });

    await runExplain();

    expect(executeDbStreamed.mock.calls[0][0].params.query).toBe(
      "EXPLAIN QUERY PLAN SELECT 1",
    );
  });

  it("does not double-wrap a query the user already explained", async () => {
    executeDbStreamed.mockResolvedValue({
      status: "success",
      response: selectResponse([{ plan: "x" }]),
    });
    const { runExplain } = setup({ body: "EXPLAIN SELECT 9;" });

    await runExplain();

    expect(executeDbStreamed.mock.calls[0][0].params.query).toBe(
      "EXPLAIN SELECT 9",
    );
  });

  it("surfaces a SQL-level error result as a panel error", async () => {
    executeDbStreamed.mockResolvedValue({
      status: "success",
      response: {
        results: [{ kind: "error", message: "syntax error" }],
        messages: [],
        stats: { elapsed_ms: 2 },
      },
    });
    const { runExplain, states, errors } = setup({});

    await runExplain();

    expect(errors.at(-1)).toBe("syntax error");
    expect(states.at(-1)).toBe("error");
  });

  it("errors when the driver returns no plan rows", async () => {
    executeDbStreamed.mockResolvedValue({
      status: "success",
      response: selectResponse([]),
    });
    const { runExplain, states, errors } = setup({});

    await runExplain();

    expect(errors.at(-1)).toContain("didn't return a plan");
    expect(states.at(-1)).toBe("error");
  });

  it("propagates an execution error", async () => {
    executeDbStreamed.mockResolvedValue({ status: "error", message: "down" });
    const { runExplain, states, errors } = setup({});

    await runExplain();

    expect(errors.at(-1)).toBe("down");
    expect(states.at(-1)).toBe("error");
  });

  it("maps a cancelled outcome to the cancelled state", async () => {
    executeDbStreamed.mockResolvedValue({ status: "cancelled" });
    const { runExplain, states } = setup({});

    await runExplain();

    expect(states.at(-1)).toBe("cancelled");
  });

  it("is a no-op while running, without a connection, or with an empty body", async () => {
    const running = setup({ executionState: "running" });
    await running.runExplain();
    const noConn = setup({ connection: null });
    await noConn.runExplain();
    const empty = setup({ body: "   " });
    await empty.runExplain();

    expect(executeDbStreamed).not.toHaveBeenCalled();
  });
});
