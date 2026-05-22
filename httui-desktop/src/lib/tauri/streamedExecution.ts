/**
 * Cancel-aware block execution over a Tauri Channel.
 * Wires `execute_db_streamed` / `execute_http_streamed` / `cancel_block`.
 */

import { Channel, invoke } from "@tauri-apps/api/core";

import type { DbResponse } from "@/components/blocks/db/types";
import { normalizeDbResponse } from "@/components/blocks/db/types";
import { useConnectionSessionOverrideStore } from "@/stores/connectionSessionOverride";

/**
 * Merge any session-scoped host:port override for this run's connection
 * into the DB params. Read via `getState` — this is
 * the single seam every DB execution funnels through, so the override
 * applies session-wide without touching the `DbFencedPanel` monolith.
 * No override → params returned unchanged.
 */
export function applyConnectionOverride(
  params: Record<string, unknown>,
): Record<string, unknown> {
  const connId = params.connection_id;
  if (typeof connId !== "string" || connId === "") return params;
  const ov = useConnectionSessionOverrideStore.getState().getOverride(connId);
  if (!ov || (ov.host === undefined && ov.port === undefined)) return params;
  const next = { ...params };
  if (ov.host !== undefined) next.session_host_override = ov.host;
  if (ov.port !== undefined) next.session_port_override = ov.port;
  return next;
}

/** Backend-emitted chunk on the execution channel. */
export type DbChunk =
  | {
      kind: "complete";
      results: unknown[];
      messages: unknown[];
      stats: { elapsed_ms: number; rows_streamed?: number | null };
      plan?: unknown;
    }
  | { kind: "error"; message: string }
  | { kind: "cancelled" };

/** Terminal outcome of a streamed execution. */
export type StreamedExecutionOutcome =
  | { status: "success"; response: DbResponse }
  | { status: "error"; message: string }
  | { status: "cancelled" };

export interface ExecuteDbStreamedOptions {
  /** Arbitrary string unique across in-flight executions. Used by `cancelBlockExecution`. */
  executionId: string;
  /** Validated DB block params (connection_id, query, etc.). */
  params: Record<string, unknown>;
  /**
   * Optional abort signal. When aborted, sends `cancel_block(executionId)` to
   * the backend. The backend responds with a `{kind: "cancelled"}` chunk.
   */
  signal?: AbortSignal;
}

/**
 * Run a DB query against the cancel-aware backend. Resolves with the final outcome.
 */
export async function executeDbStreamed(
  options: ExecuteDbStreamedOptions,
): Promise<StreamedExecutionOutcome> {
  const { executionId, params, signal } = options;

  if (signal?.aborted) {
    return { status: "cancelled" };
  }

  const channel = new Channel<DbChunk>();

  const outcome = new Promise<StreamedExecutionOutcome>((resolve) => {
    channel.onmessage = (chunk) => {
      switch (chunk.kind) {
        case "complete":
          resolve({
            status: "success",
            response: normalizeDbResponse(chunk),
          });
          break;
        case "error":
          resolve({ status: "error", message: chunk.message });
          break;
        case "cancelled":
          resolve({ status: "cancelled" });
          break;
      }
    };
  });

  const onAbort = () => {
    void cancelBlockExecution(executionId);
  };
  signal?.addEventListener("abort", onAbort, { once: true });

  try {
    await invoke<void>("execute_db_streamed", {
      params: applyConnectionOverride(params),
      executionId,
      onChunk: channel,
    });
  } catch (e) {
    signal?.removeEventListener("abort", onAbort);
    return {
      status: "error",
      message: e instanceof Error ? e.message : String(e),
    };
  }

  const result = await outcome;
  signal?.removeEventListener("abort", onAbort);
  return result;
}

/**
 * Signal cancellation for an in-flight execution by id. Returns `true` if
 * the backend found the execution in its registry; `false` if it had
 * already finished.
 */
export function cancelBlockExecution(executionId: string): Promise<boolean> {
  return invoke<boolean>("cancel_block", { executionId });
}

/** Per-execution timing breakdown emitted by the HTTP executor. */
export interface HttpTimingBreakdown {
  total_ms: number;
  dns_ms?: number | null;
  connect_ms?: number | null;
  tls_ms?: number | null;
  ttfb_ms?: number | null;
  connection_reused: boolean;
}

/** Cookie captured from a `Set-Cookie` response header. */
export interface HttpCookieRaw {
  name: string;
  value: string;
  domain?: string | null;
  path?: string | null;
  expires?: string | null;
  secure: boolean;
  http_only: boolean;
}

/**
 * Full HTTP response shape emitted by the streamed backend (`HttpChunk::Complete`).
 * Note: `body` is `unknown` — callers must narrow (parsed JSON, string, or
 * `{ encoding: "base64", data: string }` for binary).
 */
export interface HttpResponseFull {
  status_code: number;
  status_text: string;
  headers: Record<string, string>;
  body: unknown;
  size_bytes: number;
  elapsed_ms: number;
  timing: HttpTimingBreakdown;
  cookies: HttpCookieRaw[];
}

/**
 * Backend-emitted chunk on the HTTP execution channel.
 * Wire order on success: `headers` → `body_chunk` × N → `complete`.
 * The consolidated body lives in `complete`; `body_chunk` byte count drives progress.
 */
export type HttpChunk =
  | {
      kind: "headers";
      status_code: number;
      status_text: string;
      headers: Record<string, string>;
      ttfb_ms: number;
    }
  | { kind: "body_chunk"; offset: number; bytes: number[] }
  | ({ kind: "complete" } & HttpResponseFull)
  | { kind: "error"; message: string }
  | { kind: "cancelled" };

/** Terminal outcome of a streamed HTTP execution. */
export type StreamedHttpOutcome =
  | { status: "success"; response: HttpResponseFull }
  | { status: "error"; message: string }
  | { status: "cancelled" };

export interface ExecuteHttpStreamedOptions {
  /** Arbitrary string unique across in-flight executions. */
  executionId: string;
  /** Validated HTTP block params (method, url, headers, body, etc.). */
  params: Record<string, unknown>;
  /** Optional abort signal — sends `cancel_block(executionId)` when fired. */
  signal?: AbortSignal;
  /**
   * Called once when the `Headers` chunk arrives — i.e. the moment the
   * server returned the status line. Use this to flip the statusbar dot
   * to the response status class before the body finishes downloading.
   */
  onHeaders?: (headers: {
    status_code: number;
    status_text: string;
    ttfb_ms: number;
  }) => void;
  /** Called per `BodyChunk` with cumulative bytes received — drive a progress indicator. */
  onProgress?: (bytesReceived: number) => void;
}

/**
 * Run an HTTP request against the cancel-aware backend. Resolves with the
 * terminal outcome. HTTP-level errors (4xx/5xx) come back as `success` with
 * the status code preserved — only transport / cancel failures map to other
 * outcomes.
 */
export async function executeHttpStreamed(
  options: ExecuteHttpStreamedOptions,
): Promise<StreamedHttpOutcome> {
  const { executionId, params, signal } = options;

  if (signal?.aborted) {
    return { status: "cancelled" };
  }

  const channel = new Channel<HttpChunk>();

  const outcome = new Promise<StreamedHttpOutcome>((resolve) => {
    channel.onmessage = (chunk) => {
      switch (chunk.kind) {
        case "headers":
          options.onHeaders?.({
            status_code: chunk.status_code,
            status_text: chunk.status_text,
            ttfb_ms: chunk.ttfb_ms,
          });
          break;
        case "body_chunk":
          options.onProgress?.(chunk.offset + chunk.bytes.length);
          break;
        case "complete":
          resolve({
            status: "success",
            response: normalizeHttpResponse(chunk),
          });
          break;
        case "error":
          resolve({ status: "error", message: chunk.message });
          break;
        case "cancelled":
          resolve({ status: "cancelled" });
          break;
      }
    };
  });

  const onAbort = () => {
    void cancelBlockExecution(executionId);
  };
  signal?.addEventListener("abort", onAbort, { once: true });

  try {
    await invoke<void>("execute_http_streamed", {
      params,
      executionId,
      onChunk: channel,
    });
  } catch (e) {
    signal?.removeEventListener("abort", onAbort);
    return {
      status: "error",
      message: e instanceof Error ? e.message : String(e),
    };
  }

  const result = await outcome;
  signal?.removeEventListener("abort", onAbort);
  return result;
}

/**
 * Normalize a raw HTTP backend response into `HttpResponseFull`.
 * Accepts both the current shape (with `timing` + `cookies`) and the legacy
 * cached shape so older cached results keep working.
 */
export function normalizeHttpResponse(raw: unknown): HttpResponseFull {
  const obj = (raw && typeof raw === "object" ? raw : {}) as Record<
    string,
    unknown
  >;

  const status_code = typeof obj.status_code === "number" ? obj.status_code : 0;
  const status_text =
    typeof obj.status_text === "string" ? obj.status_text : "";
  const headers =
    obj.headers && typeof obj.headers === "object"
      ? (obj.headers as Record<string, string>)
      : {};
  const body = obj.body;
  const size_bytes = typeof obj.size_bytes === "number" ? obj.size_bytes : 0;
  const elapsed_ms =
    typeof obj.elapsed_ms === "number"
      ? obj.elapsed_ms
      : typeof obj.duration_ms === "number"
        ? obj.duration_ms
        : 0;

  const rawTiming =
    obj.timing && typeof obj.timing === "object"
      ? (obj.timing as Partial<HttpTimingBreakdown>)
      : null;
  const timing: HttpTimingBreakdown = {
    total_ms:
      typeof rawTiming?.total_ms === "number" ? rawTiming.total_ms : elapsed_ms,
    dns_ms: rawTiming?.dns_ms ?? null,
    connect_ms: rawTiming?.connect_ms ?? null,
    tls_ms: rawTiming?.tls_ms ?? null,
    ttfb_ms: rawTiming?.ttfb_ms ?? null,
    connection_reused: rawTiming?.connection_reused === true,
  };

  const cookies: HttpCookieRaw[] = Array.isArray(obj.cookies)
    ? (obj.cookies as HttpCookieRaw[])
    : [];

  return {
    status_code,
    status_text,
    headers,
    body,
    size_bytes,
    elapsed_ms,
    timing,
    cookies,
  };
}
