// Load-more must reuse the original query with offset = rows fetched,
// append the new page to the first select result, and dedup in-flight
// clicks via the ref guard.
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

import { useDbLoadMore } from "../useDbLoadMore";
import type { DbResponse } from "@/components/blocks/db/types";
import {
  makeConnection,
  makeDbBlock,
  makeView,
  selectResponse,
} from "./helpers";

beforeEach(() => {
  executeDbStreamed.mockReset();
});

function setup(response: DbResponse | null) {
  const view = makeView("");
  const block = makeDbBlock(view, { alias: "q1", limit: 2 });
  let current = response;
  const setResponse = vi.fn(
    (updater: (prev: DbResponse | null) => DbResponse | null) => {
      current = updater(current);
    },
  );
  const { result } = renderHook(() =>
    useDbLoadMore({
      activeConnection: makeConnection(),
      block,
      blockId: "db_idx_0",
      view,
      filePath: "f.md",
      response,
      setResponse,
    }),
  );
  return { loadMore: result.current, setResponse, get: () => current };
}

describe("useDbLoadMore", () => {
  it("appends the next page with offset = fetched rows", async () => {
    const { loadMore, get } = setup(
      selectResponse([{ id: 1 }, { id: 2 }], { hasMore: true }),
    );
    executeDbStreamed.mockResolvedValue({
      status: "success",
      response: selectResponse([{ id: 3 }], { hasMore: false }),
    });

    await loadMore();

    expect(executeDbStreamed).toHaveBeenCalledTimes(1);
    const call = executeDbStreamed.mock.calls[0][0];
    expect(call.params.offset).toBe(2);
    expect(call.params.fetch_size).toBe(2);
    const first = get()?.results[0];
    expect(first?.kind === "select" && first.rows.map((r) => r.id)).toEqual([
      1, 2, 3,
    ]);
    expect(first?.kind === "select" && first.has_more).toBe(false);
  });

  it("is a no-op when the result has no more rows", async () => {
    const { loadMore } = setup(selectResponse([{ id: 1 }], { hasMore: false }));
    await loadMore();
    expect(executeDbStreamed).not.toHaveBeenCalled();
  });

  it("is a no-op without a previous response", async () => {
    const { loadMore } = setup(null);
    await loadMore();
    expect(executeDbStreamed).not.toHaveBeenCalled();
  });

  it("ignores a failed page fetch", async () => {
    const { loadMore, setResponse } = setup(
      selectResponse([{ id: 1 }], { hasMore: true }),
    );
    executeDbStreamed.mockResolvedValue({ status: "error", message: "boom" });
    await loadMore();
    expect(setResponse).not.toHaveBeenCalled();
  });

  it("dedups concurrent clicks via the in-flight guard", async () => {
    const { loadMore } = setup(selectResponse([{ id: 1 }], { hasMore: true }));
    let release: (v: unknown) => void = () => {};
    executeDbStreamed.mockImplementation(
      () =>
        new Promise((res) => {
          release = res;
        }),
    );

    const p1 = loadMore();
    const p2 = loadMore();
    await vi.waitFor(() => expect(executeDbStreamed).toHaveBeenCalled());
    release({ status: "success", response: selectResponse([]) });
    await Promise.all([p1, p2]);

    expect(executeDbStreamed).toHaveBeenCalledTimes(1);
  });
});
