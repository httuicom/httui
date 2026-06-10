// Cache hydration must only fire for a complete (file, conn, body)
// triple and must surface the cached row's status faithfully.
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

const getBlockResult = vi.fn();
vi.mock("@/lib/tauri/commands", () => ({
  getBlockResult: (...args: unknown[]) => getBlockResult(...args),
}));
vi.mock("@/lib/blocks/hash", () => ({
  computeDbCacheHash: vi.fn(async () => "hash1"),
}));
vi.mock("@/stores/environment", () => ({
  useEnvironmentStore: {
    getState: () => ({ getActiveVariables: async () => ({}) }),
  },
}));

import { useDbCacheHydrate } from "../useDbCacheHydrate";

beforeEach(() => {
  getBlockResult.mockReset();
});

const base = { filePath: "f.md", body: "SELECT 1;", connId: "c1" };

describe("useDbCacheHydrate", () => {
  it("surfaces a cached success row", async () => {
    getBlockResult.mockResolvedValue({
      status: "success",
      response: JSON.stringify({
        results: [],
        messages: [],
        stats: { elapsed_ms: 3 },
      }),
      elapsed_ms: 3,
    });
    const onHit = vi.fn();
    renderHook(() => useDbCacheHydrate({ ...base, onHit }));

    await waitFor(() => expect(onHit).toHaveBeenCalled());
    expect(getBlockResult).toHaveBeenCalledWith("f.md", "hash1");
    expect(onHit.mock.calls[0][0]).toMatchObject({
      elapsedMs: 3,
      state: "success",
    });
  });

  it("maps a cached error row to the error state", async () => {
    getBlockResult.mockResolvedValue({
      status: "error",
      response: JSON.stringify({
        results: [],
        messages: [],
        stats: { elapsed_ms: 1 },
      }),
      elapsed_ms: null,
    });
    const onHit = vi.fn();
    renderHook(() => useDbCacheHydrate({ ...base, onHit }));

    await waitFor(() => expect(onHit).toHaveBeenCalled());
    expect(onHit.mock.calls[0][0]).toMatchObject({
      elapsedMs: null,
      state: "error",
    });
  });

  it("stays idle on cache miss", async () => {
    getBlockResult.mockResolvedValue(null);
    const onHit = vi.fn();
    renderHook(() => useDbCacheHydrate({ ...base, onHit }));

    await waitFor(() => expect(getBlockResult).toHaveBeenCalled());
    expect(onHit).not.toHaveBeenCalled();
  });

  it("stays idle on a corrupt cached response", async () => {
    getBlockResult.mockResolvedValue({
      status: "success",
      response: "not-json",
      elapsed_ms: 1,
    });
    const onHit = vi.fn();
    renderHook(() => useDbCacheHydrate({ ...base, onHit }));

    await waitFor(() => expect(getBlockResult).toHaveBeenCalled());
    expect(onHit).not.toHaveBeenCalled();
  });

  it("does not fetch without a connection or with an empty body", () => {
    const onHit = vi.fn();
    renderHook(() => useDbCacheHydrate({ ...base, connId: "", onHit }));
    renderHook(() => useDbCacheHydrate({ ...base, body: "   ", onHit }));

    expect(getBlockResult).not.toHaveBeenCalled();
  });
});
