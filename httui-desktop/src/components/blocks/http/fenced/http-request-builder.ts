/**
 * Pure module-level helpers extracted from `HttpFencedPanel.tsx` during
 * the follow-up to A1+A2a (decompose the orchestrator < 600 L). Every
 * function here is pure (no React, no Tauri) and synchronously
 * transforms a parsed HTTP message into the executor's request shape.
 *
 * Co-located under `http/fenced/` next to the other A1 siblings.
 */

import {
  parseHttpMessageBody,
  type HttpMessageParsed,
} from "@/lib/blocks/http-message";
import {
  parseLegacyHttpBody,
  legacyToHttpMessage,
} from "@/lib/blocks/http-legacy";
import type { HttpBlockSettings } from "@/lib/tauri/commands";
import type { HttpResponseFull } from "@/lib/tauri/streamedExecution";

/**
 * Parse the block's raw body to a typed request. Recognises both the
 * post-redesign HTTP-message form and the legacy JSON shim — older
 * vaults stored `{"method":"...","url":"..."}` JSON, and we convert
 * on read so saved vaults stay compatible.
 */
export function parseBody(body: string): HttpMessageParsed {
  const legacy = parseLegacyHttpBody(body);
  if (legacy) return legacyToHttpMessage(legacy);
  return parseHttpMessageBody(body);
}

/** Extract the host from a fully-qualified URL string; null on parse error. */
export function deriveHost(rawUrl: string): string | null {
  if (!rawUrl) return null;
  try {
    const u = new URL(rawUrl);
    return u.host;
  } catch {
    return null;
  }
}

/**
 * Stable (module-level) `elapsedOf` adapter for `useExecutableBlock` —
 * keeps the hook's `run` identity stable across renders (and with it
 * the `setHttpBlockActions` effect keyed on it), since a fresh closure
 * per render would otherwise force `run` to be rebuilt.
 */
export const httpElapsedOf = (r: HttpResponseFull): number | undefined =>
  r.elapsed_ms;

/**
 * RFC 7230 header-name token characters. Reqwest rejects anything outside
 * this set (notably whitespace, control chars, `{`, `}`, `(`, `)`, `,`,
 * `:`, `;`, `<`, `>`, `=`, `@`, `[`, `\`, `]`, `?`, `/`, `"`, etc).
 */
const HTTP_TOKEN_RE = /^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$/;

export function isValidHeaderName(name: string): boolean {
  return HTTP_TOKEN_RE.test(name);
}

/**
 * Build the executor params from the parsed-and-resolved request.
 *
 * `{{ref}}` is resolved in BOTH the key and the value of every header /
 * query param — keys must be resolvable too, otherwise a header name
 * like `{{auth.header_name}}` would reach reqwest verbatim and fail
 * with `builder error` (reqwest rejects `{` in header names per RFC
 * 7230).
 *
 * Rows whose key resolves to empty are dropped as a safety net so a
 * stray `headers:` label or an unresolved ref doesn't generate an
 * invalid request.
 *
 * Returns the executor params plus a list of validation errors
 * collected along the way (e.g. a header name that resolves to a
 * value containing whitespace — invalid per RFC 7230). The caller
 * surfaces these to the user instead of letting reqwest emit a
 * generic `builder error`.
 */
export function buildExecutorParams(
  parsed: HttpMessageParsed,
  resolveText: (s: string) => string,
  timeoutMs: number | undefined,
  settings: HttpBlockSettings = {},
): { params: Record<string, unknown>; errors: string[] } {
  const errors: string[] = [];

  const resolveHeaders = (rows: HttpMessageParsed["headers"]) =>
    rows
      .filter((r) => r.enabled)
      .map((r) => ({
        rawKey: r.key,
        key: resolveText(r.key).trim(),
        value: resolveText(r.value),
      }))
      .filter((r) => {
        if (r.key.length === 0) return false;
        if (!isValidHeaderName(r.key)) {
          errors.push(
            `Invalid header name "${r.key}"` +
              (r.rawKey !== r.key ? ` (resolved from "${r.rawKey}")` : "") +
              " — header names cannot contain spaces or special characters.",
          );
          return false;
        }
        return true;
      })
      .map(({ key, value }) => ({ key, value }));

  const resolveQueryParams = (rows: HttpMessageParsed["params"]) =>
    rows
      .filter((r) => r.enabled)
      .map((r) => ({
        key: resolveText(r.key).trim(),
        value: resolveText(r.value),
      }))
      .filter((r) => r.key.length > 0);

  const params: Record<string, unknown> = {
    method: parsed.method,
    url: resolveText(parsed.url),
    params: resolveQueryParams(parsed.params),
    headers: resolveHeaders(parsed.headers),
    body: parsed.body ? resolveText(parsed.body) : "",
  };
  if (timeoutMs !== undefined) params.timeout_ms = timeoutMs;
  // Per-block transport flags (Onda 1). We forward only explicit
  // overrides so the backend's defaults (true / true / true / true)
  // stay in charge when the row is absent.
  if (settings.followRedirects === false) params.follow_redirects = false;
  if (settings.verifySsl === false) params.verify_ssl = false;
  if (settings.encodeUrl === false) params.encode_url = false;
  if (settings.trimWhitespace === false) params.trim_whitespace = false;
  return { params, errors };
}
