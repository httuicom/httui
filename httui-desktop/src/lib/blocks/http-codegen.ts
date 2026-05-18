/**
 * Code generators for the HTTP block "Send as" menu.
 *
 * Pure functions — each takes an `HttpMessageParsed` whose references
 * have already been resolved by the panel and returns a snippet string
 * the user can paste into a terminal / editor / IDE.
 *
 * Encoding rules:
 *  - Disabled rows (enabled=false) are dropped — generated code never
 *    includes commented-out values.
 *  - Empty `params` / `headers` / `body` are emitted only when non-empty.
 *  - Quotes and special characters in values are escaped per target
 *    (shell single-quote, JS string literal, Python string literal).
 *  - Query params are serialized with `URLSearchParams`-style encoding
 *    (`encodeURIComponent`).
 */

import { stringifyHttpMessageBody, type HttpMessageParsed } from "./http-fence";

function enabledKV(rows: HttpMessageParsed["headers"]) {
  return rows.filter((r) => r.enabled && r.key.length > 0);
}

function buildQueryString(parsed: HttpMessageParsed): string {
  const enabled = enabledKV(parsed.params);
  if (enabled.length === 0) return "";
  return enabled
    .map((p) => `${encodeURIComponent(p.key)}=${encodeURIComponent(p.value)}`)
    .join("&");
}

function buildUrlWithQuery(parsed: HttpMessageParsed): string {
  const q = buildQueryString(parsed);
  if (!q) return parsed.url;
  const sep = parsed.url.includes("?") ? "&" : "?";
  return `${parsed.url}${sep}${q}`;
}

const METHODS_WITH_BODY: ReadonlySet<string> = new Set([
  "POST",
  "PUT",
  "PATCH",
  "DELETE",
]);

function methodHasBody(method: string, body: string): boolean {
  return METHODS_WITH_BODY.has(method) && body.length > 0;
}

// ─────────────────────── cURL ───────────────────────

/** Escape a value for inclusion inside single-quotes in POSIX shell. */
function shellSingleQuote(s: string): string {
  // Single quote → close quote, emit escaped single quote, reopen quote.
  return `'${s.replace(/'/g, "'\\''")}'`;
}

export function toCurl(parsed: HttpMessageParsed): string {
  const lines: string[] = [];
  const url = buildUrlWithQuery(parsed);
  lines.push(`curl -X ${parsed.method} ${shellSingleQuote(url)}`);
  for (const h of enabledKV(parsed.headers)) {
    lines.push(`  -H ${shellSingleQuote(`${h.key}: ${h.value}`)}`);
  }
  if (methodHasBody(parsed.method, parsed.body)) {
    lines.push(`  --data-raw ${shellSingleQuote(parsed.body)}`);
  }
  return lines.join(" \\\n");
}

// ─────────────────────── fetch (JavaScript) ───────────────────────

/** Quote a string as a single-quoted JS literal. */
function jsString(s: string): string {
  return `'${s.replace(/\\/g, "\\\\").replace(/'/g, "\\'").replace(/\n/g, "\\n")}'`;
}

export function toFetch(parsed: HttpMessageParsed): string {
  const url = buildUrlWithQuery(parsed);
  const headers = enabledKV(parsed.headers);
  const lines: string[] = [];
  lines.push(`await fetch(${jsString(url)}, {`);
  lines.push(`  method: ${jsString(parsed.method)},`);
  if (headers.length > 0) {
    lines.push(`  headers: {`);
    for (const h of headers) {
      lines.push(`    ${jsString(h.key)}: ${jsString(h.value)},`);
    }
    lines.push(`  },`);
  }
  if (methodHasBody(parsed.method, parsed.body)) {
    lines.push(`  body: ${jsString(parsed.body)},`);
  }
  lines.push(`});`);
  return lines.join("\n");
}

// ─────────────────────── Python requests ───────────────────────

/** Quote as a Python single-quoted string. */
function pyString(s: string): string {
  return `'${s.replace(/\\/g, "\\\\").replace(/'/g, "\\'").replace(/\n/g, "\\n")}'`;
}

export function toPython(parsed: HttpMessageParsed): string {
  const lines: string[] = [];
  lines.push("import requests");
  lines.push("");
  const fnName = parsed.method.toLowerCase();
  lines.push(`response = requests.${fnName}(`);
  lines.push(`    ${pyString(parsed.url)},`);
  const params = enabledKV(parsed.params);
  if (params.length > 0) {
    lines.push(`    params={`);
    for (const p of params) {
      lines.push(`        ${pyString(p.key)}: ${pyString(p.value)},`);
    }
    lines.push(`    },`);
  }
  const headers = enabledKV(parsed.headers);
  if (headers.length > 0) {
    lines.push(`    headers={`);
    for (const h of headers) {
      lines.push(`        ${pyString(h.key)}: ${pyString(h.value)},`);
    }
    lines.push(`    },`);
  }
  if (methodHasBody(parsed.method, parsed.body)) {
    lines.push(`    data=${pyString(parsed.body)},`);
  }
  lines.push(`)`);
  return lines.join("\n");
}

// ─────────────────────── HTTPie ───────────────────────

/**
 * HTTPie request item syntax:
 *   - `key=value`        → JSON body / form field
 *   - `key==value`       → query parameter
 *   - `Header:value`     → request header
 * Values containing spaces or shell metacharacters are single-quoted as a
 * whole token. Body is passed via stdin via `echo … | http …` only if it's
 * not a flat key=value JSON — for the simple cases we let HTTPie infer the
 * JSON body from the request items. For non-trivial bodies we fall back
 * to `--raw` (HTTPie 3.0+) which accepts the body as a string flag.
 */
export function toHTTPie(parsed: HttpMessageParsed): string {
  const tokens: string[] = ["http", parsed.method];
  tokens.push(shellSingleQuote(parsed.url));

  for (const p of enabledKV(parsed.params)) {
    // == is the HTTPie syntax for query params.
    tokens.push(shellSingleQuote(`${p.key}==${p.value}`));
  }
  for (const h of enabledKV(parsed.headers)) {
    tokens.push(shellSingleQuote(`${h.key}:${h.value}`));
  }
  if (methodHasBody(parsed.method, parsed.body)) {
    tokens.push(`--raw=${shellSingleQuote(parsed.body)}`);
  }
  return tokens.join(" ");
}

// ─────────────────────── .http file ───────────────────────

/**
 * Emit the canonical HTTP-message body the user can paste into a `.http` /
 * `.rest` file (REST Client extension, JetBrains HTTP Client, etc).
 * One request per file — multi-request files separated by `###` aren't
 * generated here; that's a V2 if there's demand.
 */
export function toHttpFile(parsed: HttpMessageParsed): string {
  return stringifyHttpMessageBody(parsed) + "\n";
}
