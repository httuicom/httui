import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import {
  normalizeHttpResponse,
  applyConnectionOverride,
  executeDbStreamed,
  executeHttpStreamed,
  cancelBlockExecution,
} from "../streamedExecution";
import { useConnectionSessionOverrideStore } from "@/stores/connectionSessionOverride";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";

type ChunkSink = { onmessage: ((m: unknown) => void) | null };

/** Register a mocked streamed command that pushes the given chunks
 * into the channel as soon as the command is invoked. */
function streamChunks(cmd: string, chunks: unknown[]) {
  mockTauriCommand(cmd, (args) => {
    const ch = (args as { onChunk: ChunkSink }).onChunk;
    for (const c of chunks) ch.onmessage?.(c);
    return undefined;
  });
}

describe("normalizeHttpResponse", () => {
  it("accepts the new full shape verbatim", () => {
    const raw = {
      status_code: 200,
      status_text: "OK",
      headers: { "content-type": "application/json" },
      body: { hello: "world" },
      size_bytes: 17,
      elapsed_ms: 42,
      timing: { total_ms: 42, dns_ms: 5 },
      cookies: [
        {
          name: "sid",
          value: "abc",
          secure: true,
          http_only: false,
        },
      ],
    };
    const out = normalizeHttpResponse(raw);
    expect(out.status_code).toBe(200);
    expect(out.body).toEqual({ hello: "world" });
    expect(out.timing.total_ms).toBe(42);
    expect(out.timing.dns_ms).toBe(5);
    expect(out.cookies).toHaveLength(1);
    expect(out.cookies[0].name).toBe("sid");
  });

  it("synthesizes timing from elapsed_ms when missing", () => {
    const raw = {
      status_code: 200,
      status_text: "OK",
      headers: {},
      body: "",
      size_bytes: 0,
      elapsed_ms: 123,
    };
    const out = normalizeHttpResponse(raw);
    expect(out.timing.total_ms).toBe(123);
    // V2 sub-fields stay nullish until isahc swap.
    expect(out.timing.dns_ms ?? null).toBeNull();
    expect(out.timing.connect_ms ?? null).toBeNull();
    expect(out.timing.connection_reused).toBe(false);
    expect(out.cookies).toEqual([]);
  });

  it("falls back to duration_ms (legacy cached shape)", () => {
    const raw = {
      status_code: 404,
      status_text: "Not Found",
      headers: {},
      body: "Not Found",
      size_bytes: 9,
      duration_ms: 18,
    };
    const out = normalizeHttpResponse(raw);
    expect(out.elapsed_ms).toBe(18);
    expect(out.timing.total_ms).toBe(18);
  });

  it("returns sane defaults for completely empty input", () => {
    const out = normalizeHttpResponse({});
    expect(out.status_code).toBe(0);
    expect(out.status_text).toBe("");
    expect(out.headers).toEqual({});
    expect(out.body).toBeUndefined();
    expect(out.size_bytes).toBe(0);
    expect(out.elapsed_ms).toBe(0);
    expect(out.timing.total_ms).toBe(0);
    expect(out.timing.connection_reused).toBe(false);
    expect(out.cookies).toEqual([]);
  });

  it("ignores invalid types instead of throwing", () => {
    const out = normalizeHttpResponse({
      status_code: "200",
      headers: "not an object",
      cookies: "nope",
      timing: 42,
    });
    expect(out.status_code).toBe(0);
    expect(out.headers).toEqual({});
    expect(out.cookies).toEqual([]);
    expect(out.timing.total_ms).toBe(0);
    expect(out.timing.connection_reused).toBe(false);
  });

  it("handles non-object roots", () => {
    expect(normalizeHttpResponse(null).status_code).toBe(0);
    expect(normalizeHttpResponse(undefined).status_code).toBe(0);
    expect(normalizeHttpResponse("string").status_code).toBe(0);
  });

  it("preserves Onda 4 timing fields (ttfb_ms + connection_reused)", () => {
    const raw = {
      status_code: 200,
      status_text: "OK",
      headers: {},
      body: "ok",
      size_bytes: 2,
      elapsed_ms: 150,
      timing: {
        total_ms: 150,
        ttfb_ms: 42,
        connection_reused: false,
      },
      cookies: [],
    };
    const out = normalizeHttpResponse(raw);
    expect(out.timing.ttfb_ms).toBe(42);
    expect(out.timing.connection_reused).toBe(false);
    // V2 sub-fields stay nullish.
    expect(out.timing.dns_ms ?? null).toBeNull();
    expect(out.timing.connect_ms ?? null).toBeNull();
    expect(out.timing.tls_ms ?? null).toBeNull();
  });

  it("defaults connection_reused to false for legacy cached shapes", () => {
    // Pre-Onda-4 cached responses don't carry `connection_reused`. The
    // normalizer must fill it in so consumers always see a boolean.
    const raw = {
      status_code: 200,
      status_text: "OK",
      headers: {},
      body: "ok",
      size_bytes: 2,
      elapsed_ms: 50,
      timing: { total_ms: 50 },
      cookies: [],
    };
    const out = normalizeHttpResponse(raw);
    expect(out.timing.connection_reused).toBe(false);
    expect(typeof out.timing.connection_reused).toBe("boolean");
  });
});

describe("applyConnectionOverride", () => {
  beforeEach(() => {
    useConnectionSessionOverrideStore.setState({ overrides: {} });
  });

  it("returns params unchanged when no override is set", () => {
    const params = { connection_id: "c1", query: "SELECT 1" };
    expect(applyConnectionOverride(params)).toBe(params);
  });

  it("returns params unchanged when connection_id is missing/blank", () => {
    useConnectionSessionOverrideStore
      .getState()
      .setOverride("c1", { host: "h" });
    const p1 = { query: "x" };
    expect(applyConnectionOverride(p1)).toBe(p1);
    const p2 = { connection_id: "", query: "x" };
    expect(applyConnectionOverride(p2)).toBe(p2);
  });

  it("merges host + port override into a fresh params object", () => {
    useConnectionSessionOverrideStore
      .getState()
      .setOverride("c1", { host: "db.staging", port: 5599 });
    const params = { connection_id: "c1", query: "SELECT 1" };
    const out = applyConnectionOverride(params);
    expect(out).not.toBe(params);
    expect(out).toMatchObject({
      connection_id: "c1",
      query: "SELECT 1",
      session_host_override: "db.staging",
      session_port_override: 5599,
    });
    // Original is not mutated.
    expect(params).not.toHaveProperty("session_host_override");
  });

  it("includes only the field that was overridden", () => {
    useConnectionSessionOverrideStore
      .getState()
      .setOverride("c1", { port: 6000 });
    const out = applyConnectionOverride({ connection_id: "c1" });
    expect(out.session_port_override).toBe(6000);
    expect(out).not.toHaveProperty("session_host_override");
  });

  it("does not apply another connection's override", () => {
    useConnectionSessionOverrideStore
      .getState()
      .setOverride("other", { host: "h" });
    const params = { connection_id: "c1" };
    expect(applyConnectionOverride(params)).toBe(params);
  });
});

describe("executeDbStreamed", () => {
  beforeEach(() => {
    clearTauriMocks();
    useConnectionSessionOverrideStore.setState({ overrides: {} });
  });
  afterEach(() => clearTauriMocks());

  it("resolves success with the normalized response on complete", async () => {
    streamChunks("execute_db_streamed", [
      {
        kind: "complete",
        results: [],
        messages: [],
        stats: { elapsed_ms: 7 },
      },
    ]);
    const out = await executeDbStreamed({
      executionId: "e1",
      params: { connection_id: "c1", query: "SELECT 1" },
    });
    expect(out.status).toBe("success");
    if (out.status === "success") {
      expect(out.response.stats.elapsed_ms).toBe(7);
    }
  });

  it("maps an error chunk to an error outcome", async () => {
    streamChunks("execute_db_streamed", [{ kind: "error", message: "boom" }]);
    const out = await executeDbStreamed({
      executionId: "e1",
      params: { connection_id: "c1", query: "x" },
    });
    expect(out).toEqual({ status: "error", message: "boom" });
  });

  it("maps a cancelled chunk to a cancelled outcome", async () => {
    streamChunks("execute_db_streamed", [{ kind: "cancelled" }]);
    const out = await executeDbStreamed({
      executionId: "e1",
      params: { connection_id: "c1", query: "x" },
    });
    expect(out).toEqual({ status: "cancelled" });
  });

  it("returns cancelled without invoking when the signal is pre-aborted", async () => {
    const spy = vi.fn();
    mockTauriCommand("execute_db_streamed", spy);
    const ctrl = new AbortController();
    ctrl.abort();
    const out = await executeDbStreamed({
      executionId: "e1",
      params: { connection_id: "c1", query: "x" },
      signal: ctrl.signal,
    });
    expect(out).toEqual({ status: "cancelled" });
    expect(spy).not.toHaveBeenCalled();
  });

  it("maps a thrown backend command to an error outcome", async () => {
    mockTauriCommand("execute_db_streamed", () => {
      throw new Error("pool down");
    });
    const out = await executeDbStreamed({
      executionId: "e1",
      params: { connection_id: "c1", query: "x" },
    });
    expect(out).toEqual({ status: "error", message: "pool down" });
  });

  it("injects a session host:port override into the invoked params", async () => {
    useConnectionSessionOverrideStore
      .getState()
      .setOverride("c1", { host: "db.staging", port: 5599 });
    let seen: Record<string, unknown> | undefined;
    mockTauriCommand("execute_db_streamed", (args) => {
      seen = (args as { params: Record<string, unknown> }).params;
      (
        args as { onChunk: { onmessage: (m: unknown) => void } }
      ).onChunk.onmessage({
        kind: "complete",
        results: [],
        messages: [],
        stats: { elapsed_ms: 1 },
      });
    });
    await executeDbStreamed({
      executionId: "e1",
      params: { connection_id: "c1", query: "x" },
    });
    expect(seen).toMatchObject({
      session_host_override: "db.staging",
      session_port_override: 5599,
    });
  });

  it("fires cancel_block when the signal aborts", async () => {
    const cancelSpy = vi.fn(() => true);
    mockTauriCommand("cancel_block", cancelSpy);
    const ctrl = new AbortController();
    mockTauriCommand("execute_db_streamed", (args) => {
      // Abort mid-flight, then end the stream so the promise resolves.
      ctrl.abort();
      (
        args as { onChunk: { onmessage: (m: unknown) => void } }
      ).onChunk.onmessage({ kind: "cancelled" });
    });
    const out = await executeDbStreamed({
      executionId: "e9",
      params: { connection_id: "c1", query: "x" },
      signal: ctrl.signal,
    });
    expect(out).toEqual({ status: "cancelled" });
    expect(cancelSpy).toHaveBeenCalledWith({ executionId: "e9" });
  });
});

describe("executeHttpStreamed", () => {
  beforeEach(() => clearTauriMocks());
  afterEach(() => clearTauriMocks());

  it("calls onHeaders + onProgress and resolves the complete body", async () => {
    streamChunks("execute_http_streamed", [
      {
        kind: "headers",
        status_code: 200,
        status_text: "OK",
        ttfb_ms: 12,
      },
      { kind: "body_chunk", offset: 0, bytes: [1, 2, 3] },
      {
        kind: "complete",
        status_code: 200,
        status_text: "OK",
        headers: {},
        body: "ok",
        size_bytes: 2,
        elapsed_ms: 30,
        timing: { total_ms: 30 },
        cookies: [],
      },
    ]);
    const onHeaders = vi.fn();
    const onProgress = vi.fn();
    const out = await executeHttpStreamed({
      executionId: "h1",
      params: { method: "GET", url: "https://x" },
      onHeaders,
      onProgress,
    });
    expect(onHeaders).toHaveBeenCalledWith({
      status_code: 200,
      status_text: "OK",
      ttfb_ms: 12,
    });
    expect(onProgress).toHaveBeenCalledWith(3);
    expect(out.status).toBe("success");
  });

  it("maps error / cancelled chunks", async () => {
    streamChunks("execute_http_streamed", [
      { kind: "error", message: "dns fail" },
    ]);
    expect(
      await executeHttpStreamed({
        executionId: "h1",
        params: {},
      }),
    ).toEqual({ status: "error", message: "dns fail" });

    streamChunks("execute_http_streamed", [{ kind: "cancelled" }]);
    expect(
      await executeHttpStreamed({ executionId: "h2", params: {} }),
    ).toEqual({ status: "cancelled" });
  });

  it("pre-aborted signal short-circuits to cancelled", async () => {
    const spy = vi.fn();
    mockTauriCommand("execute_http_streamed", spy);
    const ctrl = new AbortController();
    ctrl.abort();
    const out = await executeHttpStreamed({
      executionId: "h1",
      params: {},
      signal: ctrl.signal,
    });
    expect(out).toEqual({ status: "cancelled" });
    expect(spy).not.toHaveBeenCalled();
  });

  it("maps a thrown backend command to an error outcome", async () => {
    mockTauriCommand("execute_http_streamed", () => {
      throw new Error("transport");
    });
    expect(
      await executeHttpStreamed({ executionId: "h1", params: {} }),
    ).toEqual({ status: "error", message: "transport" });
  });
});

describe("cancelBlockExecution", () => {
  beforeEach(() => clearTauriMocks());
  afterEach(() => clearTauriMocks());

  it("invokes cancel_block and returns the backend boolean", async () => {
    mockTauriCommand("cancel_block", (args) => {
      expect(args).toEqual({ executionId: "e1" });
      return true;
    });
    await expect(cancelBlockExecution("e1")).resolves.toBe(true);
  });
});
