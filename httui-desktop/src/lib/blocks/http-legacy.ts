/**
 * Pre-redesign JSON-body HTTP blocks, still supported on read:
 *
 *   ```http alias=req1 displayMode=split
 *   {"method":"POST","url":"...","params":[...],"headers":[...],"body":"..."}
 *   ```
 *
 * Used only during the retrocompat migration window — blocks are
 * converted to the HTTP-message format the first time they are written
 * back.
 */

import type { HttpMethod, HttpMessageParsed } from "./http-message";

const HTTP_METHODS: ReadonlySet<string> = new Set([
  "GET",
  "POST",
  "PUT",
  "PATCH",
  "DELETE",
  "HEAD",
  "OPTIONS",
]);

/** Shape extracted from a legacy JSON-body http block. */
export interface LegacyHttpBody {
  method: HttpMethod;
  url: string;
  params: Array<{ key: string; value: string }>;
  headers: Array<{ key: string; value: string }>;
  body: string;
  timeoutMs?: number;
}

/**
 * Detects whether a fenced block body is the pre-redesign JSON shape.
 * Heuristic: trimmed body starts with `{` AND parses as JSON with string
 * `method` and `url` fields.
 */
export function isLegacyHttpBody(body: string): boolean {
  return parseLegacyHttpBody(body) !== null;
}

/**
 * Parse a legacy JSON body. Returns null if body is not legacy-shaped.
 * Accepts both snake_case (backend) and camelCase (frontend) field names.
 */
export function parseLegacyHttpBody(body: string): LegacyHttpBody | null {
  const trimmed = body.trimStart();
  if (!trimmed.startsWith("{")) return null;

  let parsed: unknown;
  try {
    parsed = JSON.parse(trimmed);
  } catch {
    return null;
  }
  if (!parsed || typeof parsed !== "object") return null;

  const obj = parsed as Record<string, unknown>;
  if (typeof obj.method !== "string" || typeof obj.url !== "string")
    return null;

  const methodUpper = obj.method.toUpperCase();
  if (!HTTP_METHODS.has(methodUpper)) return null;

  const out: LegacyHttpBody = {
    method: methodUpper as HttpMethod,
    url: obj.url,
    params: normalizeKVArray(obj.params),
    headers: normalizeKVArray(obj.headers),
    body: typeof obj.body === "string" ? obj.body : "",
  };

  const timeout =
    typeof obj.timeout_ms === "number"
      ? obj.timeout_ms
      : typeof obj.timeoutMs === "number"
        ? obj.timeoutMs
        : undefined;
  if (timeout !== undefined && Number.isFinite(timeout)) {
    out.timeoutMs = Math.trunc(timeout);
  }

  return out;
}

function normalizeKVArray(
  value: unknown,
): Array<{ key: string; value: string }> {
  if (!Array.isArray(value)) return [];
  const out: Array<{ key: string; value: string }> = [];
  for (const item of value) {
    if (!item || typeof item !== "object") continue;
    const obj = item as Record<string, unknown>;
    const key = typeof obj.key === "string" ? obj.key : "";
    const v = typeof obj.value === "string" ? obj.value : "";
    if (key.length === 0) continue;
    out.push({ key, value: v });
  }
  return out;
}

/**
 * Convert a legacy body to the new HTTP-message shape. All rows enabled, no
 * descriptions (legacy format never had them).
 */
export function legacyToHttpMessage(legacy: LegacyHttpBody): HttpMessageParsed {
  return {
    method: legacy.method,
    url: legacy.url,
    params: legacy.params.map((p) => ({ ...p, enabled: true })),
    headers: legacy.headers.map((h) => ({ ...h, enabled: true })),
    body: legacy.body,
  };
}
