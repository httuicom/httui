import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

import { useHttpCacheHydrate } from "../useHttpCacheHydrate";
import type { HttpMessageParsed } from "@/lib/blocks/http-message";

const getBlockResultMock = vi.fn();
vi.mock("@/lib/tauri/commands", () => ({
  getBlockResult: (...args: unknown[]) => getBlockResultMock(...args),
}));

const computeHttpCacheHashMock = vi.fn();
vi.mock("@/lib/blocks/hash", () => ({
  computeHttpCacheHash: (...args: unknown[]) =>
    computeHttpCacheHashMock(...args),
}));

vi.mock("@/lib/tauri/streamedExecution", () => ({
  // Identity normalizer keeps the test focused on the cache path.
  normalizeHttpResponse: (r: unknown) => r,
}));

const getActiveVariablesMock = vi.fn();
vi.mock("@/stores/environment", () => ({
  useEnvironmentStore: {
    getState: () => ({
      getActiveVariables: () => getActiveVariablesMock(),
    }),
  },
}));

const baseParsed = (
  overrides: Partial<HttpMessageParsed> = {},
): HttpMessageParsed => ({
  method: "GET",
  url: "https://api.example.com/users",
  params: [],
  headers: [],
  body: "",
  ...overrides,
});

describe("useHttpCacheHydrate", () => {
  beforeEach(() => {
    getBlockResultMock.mockReset();
    computeHttpCacheHashMock.mockReset();
    getActiveVariablesMock.mockReset();
  });

  it("returns early for mutation methods — no IPC calls", () => {
    const apply = vi.fn();
    const setLast = vi.fn();
    renderHook(() =>
      useHttpCacheHydrate({
        parsed: baseParsed({ method: "POST" }),
        filePath: "x.md",
        applyCachedResult: apply,
        setLastRunAt: setLast,
      }),
    );
    expect(getBlockResultMock).not.toHaveBeenCalled();
    expect(getActiveVariablesMock).not.toHaveBeenCalled();
  });

  it("returns early for empty / whitespace URL", () => {
    const apply = vi.fn();
    const setLast = vi.fn();
    renderHook(() =>
      useHttpCacheHydrate({
        parsed: baseParsed({ url: "   " }),
        filePath: "x.md",
        applyCachedResult: apply,
        setLastRunAt: setLast,
      }),
    );
    expect(getBlockResultMock).not.toHaveBeenCalled();
  });

  it("hits cache: hydrates FSM + lastRunAt", async () => {
    getActiveVariablesMock.mockResolvedValue({});
    computeHttpCacheHashMock.mockResolvedValue("hash-1");
    getBlockResultMock.mockResolvedValue({
      response: JSON.stringify({ status_code: 200, elapsed_ms: 0 }),
      elapsed_ms: 42,
      executed_at: "2026-05-19T10:00:00Z",
    });
    const apply = vi.fn();
    const setLast = vi.fn();

    renderHook(() =>
      useHttpCacheHydrate({
        parsed: baseParsed(),
        filePath: "current.md",
        applyCachedResult: apply,
        setLastRunAt: setLast,
      }),
    );

    await waitFor(() => expect(apply).toHaveBeenCalled());
    expect(apply).toHaveBeenCalledWith(
      expect.objectContaining({ status_code: 200 }),
      // norm.elapsed_ms=0 falsy → falls back to hit.elapsed_ms=42.
      42,
    );
    expect(setLast).toHaveBeenCalledWith(new Date("2026-05-19T10:00:00Z"));
  });

  it("prefers normalized elapsed_ms when present", async () => {
    getActiveVariablesMock.mockResolvedValue({});
    computeHttpCacheHashMock.mockResolvedValue("h");
    getBlockResultMock.mockResolvedValue({
      response: JSON.stringify({ status_code: 200, elapsed_ms: 999 }),
      elapsed_ms: 1,
      executed_at: null,
    });
    const apply = vi.fn();
    renderHook(() =>
      useHttpCacheHydrate({
        parsed: baseParsed(),
        filePath: "x.md",
        applyCachedResult: apply,
        setLastRunAt: vi.fn(),
      }),
    );
    await waitFor(() => expect(apply).toHaveBeenCalled());
    expect(apply.mock.calls[0][1]).toBe(999);
  });

  it("no-op when getBlockResult returns null (cache miss)", async () => {
    getActiveVariablesMock.mockResolvedValue({});
    computeHttpCacheHashMock.mockResolvedValue("h");
    getBlockResultMock.mockResolvedValue(null);
    const apply = vi.fn();
    const setLast = vi.fn();
    renderHook(() =>
      useHttpCacheHydrate({
        parsed: baseParsed(),
        filePath: "x.md",
        applyCachedResult: apply,
        setLastRunAt: setLast,
      }),
    );
    // Allow async settle.
    await new Promise((r) => setTimeout(r, 5));
    expect(apply).not.toHaveBeenCalled();
    expect(setLast).not.toHaveBeenCalled();
  });

  it("swallows corrupt JSON in the cached response", async () => {
    getActiveVariablesMock.mockResolvedValue({});
    computeHttpCacheHashMock.mockResolvedValue("h");
    getBlockResultMock.mockResolvedValue({
      response: "{ this is { not json",
      elapsed_ms: 0,
      executed_at: null,
    });
    const apply = vi.fn();
    const setLast = vi.fn();
    renderHook(() =>
      useHttpCacheHydrate({
        parsed: baseParsed(),
        filePath: "x.md",
        applyCachedResult: apply,
        setLastRunAt: setLast,
      }),
    );
    await new Promise((r) => setTimeout(r, 5));
    expect(apply).not.toHaveBeenCalled();
  });

  it("swallows IPC errors (best-effort cache lookup)", async () => {
    getActiveVariablesMock.mockRejectedValue(new Error("env blew up"));
    const apply = vi.fn();
    renderHook(() =>
      useHttpCacheHydrate({
        parsed: baseParsed(),
        filePath: "x.md",
        applyCachedResult: apply,
        setLastRunAt: vi.fn(),
      }),
    );
    await new Promise((r) => setTimeout(r, 5));
    expect(apply).not.toHaveBeenCalled();
  });

  it("forwards enabled-filtered params + headers to the hash", async () => {
    getActiveVariablesMock.mockResolvedValue({ E: "1" });
    computeHttpCacheHashMock.mockResolvedValue("h");
    getBlockResultMock.mockResolvedValue(null);
    renderHook(() =>
      useHttpCacheHydrate({
        parsed: baseParsed({
          params: [
            { key: "a", value: "1", enabled: true },
            { key: "b", value: "2", enabled: false },
          ],
          headers: [
            { key: "X-A", value: "a", enabled: true },
            { key: "X-B", value: "b", enabled: false },
          ],
          body: "payload",
        }),
        filePath: "x.md",
        applyCachedResult: vi.fn(),
        setLastRunAt: vi.fn(),
      }),
    );
    await waitFor(() => expect(computeHttpCacheHashMock).toHaveBeenCalled());
    const [arg, envArg] = computeHttpCacheHashMock.mock.calls[0];
    expect(arg.params).toEqual([{ key: "a", value: "1" }]);
    expect(arg.headers).toEqual([{ key: "X-A", value: "a" }]);
    expect(arg.body).toBe("payload");
    expect(envArg).toEqual({ E: "1" });
  });

  it("passes null to setLastRunAt when executed_at is empty", async () => {
    getActiveVariablesMock.mockResolvedValue({});
    computeHttpCacheHashMock.mockResolvedValue("h");
    getBlockResultMock.mockResolvedValue({
      response: JSON.stringify({ status_code: 200 }),
      elapsed_ms: 10,
      executed_at: null,
    });
    const setLast = vi.fn();
    renderHook(() =>
      useHttpCacheHydrate({
        parsed: baseParsed(),
        filePath: "x.md",
        applyCachedResult: vi.fn(),
        setLastRunAt: setLast,
      }),
    );
    await waitFor(() => expect(setLast).toHaveBeenCalled());
    expect(setLast).toHaveBeenCalledWith(null);
  });
});
