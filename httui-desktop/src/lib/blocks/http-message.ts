/**
 * Parser and serializer for HTTP block fenced-code info strings and bodies.
 *
 * Format:
 *   ```http alias=req1 timeout=30000 display=split mode=raw
 *   POST https://api.example.com/users?page=1
 *   &limit=10
 *   Authorization: Bearer {{TOKEN}}
 *   Content-Type: application/json
 *
 *   {"name":"alice"}
 *   ```
 *
 * Info string rules:
 * - Tokens separated by whitespace; `key=value` (no spaces, no quotes — MVP).
 * - Order does not matter on read; canonical order on write is:
 *   `alias → timeout → display → mode`.
 * - Unknown keys are ignored silently. Invalid values are ignored silently.
 *
 * Body rules (HTTP message format):
 * - First non-empty, non-comment line: `METHOD URL`. URL may include inline query.
 * - Lines starting with `?` or `&` are query continuations.
 * - Until the first blank line: headers in `Key: Value` form.
 * - After the first blank line: body (until end of fence).
 * - Convention: `# desc: <text>` (case-sensitive, exactly one space) attaches a
 *   description to the line below. Other `# ...` lines = disabled (param/header
 *   commented out).
 *
 * Reading is driven by the @httui/lezer-http grammar (the lexical mirror
 * of the canonical tree-sitter grammar): line classification comes from
 * the parse tree, row contents from the line text. Lines the grammar
 * could not classify (error recovery) fall back to raw header rows so
 * the user still sees them.
 *
 * Stringifier is canonical and idempotent: `parse → stringify → parse → stringify`
 * is a fixed point.
 */

import { parser } from "@httui/lezer-http";

export type HttpMethod =
  | "GET"
  | "POST"
  | "PUT"
  | "PATCH"
  | "DELETE"
  | "HEAD"
  | "OPTIONS";

export type HttpDisplayMode = "input" | "split" | "output";

export type HttpFenceMode = "raw" | "form";

export interface HttpBlockMetadata {
  alias?: string;
  timeoutMs?: number;
  displayMode?: HttpDisplayMode;
  mode?: HttpFenceMode;
}

export interface HttpKVRow {
  key: string;
  value: string;
  enabled: boolean;
  description?: string;
}

export interface HttpMessageParsed {
  method: HttpMethod;
  url: string;
  params: HttpKVRow[];
  headers: HttpKVRow[];
  body: string;
}

const HTTP_METHODS: ReadonlySet<HttpMethod> = new Set([
  "GET",
  "POST",
  "PUT",
  "PATCH",
  "DELETE",
  "HEAD",
  "OPTIONS",
]);

const DISPLAY_MODES: readonly HttpDisplayMode[] = ["input", "split", "output"];
const FENCE_MODES: readonly HttpFenceMode[] = ["raw", "form"];

const URL_INLINE_LIMIT = 80;

// ─────────────────────── Info string ───────────────────────

/**
 * Parse a fenced-code info string into structured metadata.
 * Returns null if the head token is not `http`.
 */
export function parseHttpFenceInfo(info: string): HttpBlockMetadata | null {
  const parts = info.trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0) return null;
  if (parts[0] !== "http") return null;

  const meta: HttpBlockMetadata = {};

  for (let i = 1; i < parts.length; i++) {
    const part = parts[i];
    const eq = part.indexOf("=");
    if (eq <= 0) continue;
    const key = part.slice(0, eq);
    const value = part.slice(eq + 1);

    switch (key) {
      case "alias":
        if (value.length > 0) meta.alias = value;
        break;
      case "timeout": {
        const n = Number(value);
        if (Number.isFinite(n) && n >= 0) meta.timeoutMs = Math.trunc(n);
        break;
      }
      case "display":
      case "displayMode":
        if ((DISPLAY_MODES as readonly string[]).includes(value)) {
          meta.displayMode = value as HttpDisplayMode;
        }
        break;
      case "mode":
        if ((FENCE_MODES as readonly string[]).includes(value)) {
          meta.mode = value as HttpFenceMode;
        }
        break;
    }
  }

  return meta;
}

/**
 * Serialize metadata into a canonical info string.
 * Order is fixed: alias → timeout → display → mode.
 */
export function stringifyHttpFenceInfo(meta: HttpBlockMetadata): string {
  const parts: string[] = ["http"];
  if (meta.alias !== undefined) parts.push(`alias=${meta.alias}`);
  if (meta.timeoutMs !== undefined) parts.push(`timeout=${meta.timeoutMs}`);
  if (meta.displayMode !== undefined) parts.push(`display=${meta.displayMode}`);
  if (meta.mode !== undefined) parts.push(`mode=${meta.mode}`);
  return parts.join(" ");
}

// ─────────────────────── Body parsing ───────────────────────

const DESC_PREFIX = "# desc: ";

function emptyParse(): HttpMessageParsed {
  return { method: "GET", url: "", params: [], headers: [], body: "" };
}

function lineIndexOf(starts: number[], offset: number): number {
  let lo = 0;
  let hi = starts.length - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (starts[mid] <= offset) lo = mid;
    else hi = mid - 1;
  }
  return lo;
}

/** Per-line classification from the grammar's parse tree. */
function classifyLines(body: string, lines: string[]): Map<number, string> {
  const starts: number[] = [0];
  for (let i = 0; i < lines.length - 1; i++) {
    starts.push(starts[i] + lines[i].length + 1);
  }
  const types = new Map<number, string>();
  const tree = parser.parse(body);
  for (let node = tree.topNode.firstChild; node; node = node.nextSibling) {
    if (node.name === "Body") continue;
    types.set(lineIndexOf(starts, node.from), node.name);
  }
  return types;
}

/**
 * Parse an HTTP-message-formatted body. Always succeeds; malformed lines are
 * preserved as raw (e.g., a header without a colon stays as a free-form row
 * in `headers` with empty key).
 */
export function parseHttpMessageBody(body: string): HttpMessageParsed {
  const lines = body.split("\n");
  const types = classifyLines(body, lines);

  // The request line is the first non-blank, non-comment line; `# desc:`
  // never applies to it. A line that does not parse as `METHOD URL`
  // (unknown or lowercase method) bails out with the empty shape so the
  // caller can surface a syntax error instead of crashing.
  let i = 0;
  while (
    i < lines.length &&
    (lines[i].trim() === "" || lines[i].trim().startsWith("#"))
  ) {
    i++;
  }
  if (i >= lines.length) return emptyParse();
  if (types.get(i) !== "RequestLine") return emptyParse();
  const firstLineParsed = parseFirstLine(lines[i].trim());
  if (!firstLineParsed) return emptyParse();
  const { method, url, inlineQuery } = firstLineParsed;
  i++;

  const params: HttpKVRow[] = [];
  if (inlineQuery.length > 0) {
    for (const seg of inlineQuery.split("&")) {
      if (seg.length === 0) continue;
      const row = parseQuerySegment(seg, true, undefined);
      if (row) params.push(row);
    }
  }

  // Middle section: query continuations and headers until the first
  // blank line. Classification comes from the grammar; the semantic
  // rule "query continuations only before the first header" stays here
  // (the grammar accepts them anywhere by design).
  const headers: HttpKVRow[] = [];
  let sawHeader = false;
  let pendingDescription: string | undefined;

  while (i < lines.length) {
    const raw = lines[i];
    const trimmed = raw.trim();

    if (trimmed === "") {
      i++;
      break;
    }

    const type = types.get(i);

    if (type === "DescLine") {
      pendingDescription = trimmed.slice(DESC_PREFIX.length);
      i++;
      continue;
    }

    if (type === "DisabledQueryLine" || type === "DisabledHeaderLine") {
      const inner = trimmed === "#" ? "" : trimmed.slice(2);
      if (type === "DisabledQueryLine") {
        const row = parseQuerySegment(
          inner.slice(1),
          false,
          pendingDescription,
        );
        if (row) params.push(row);
      } else {
        const row = parseHeaderLine(inner, false, pendingDescription);
        if (row) {
          headers.push(row);
          sawHeader = true;
        }
      }
      pendingDescription = undefined;
      i++;
      continue;
    }

    if (type === "CommentLine") {
      pendingDescription = undefined;
      i++;
      continue;
    }

    if (type === "QueryLine" && !sawHeader) {
      const row = parseQuerySegment(trimmed.slice(1), true, pendingDescription);
      if (row) params.push(row);
      pendingDescription = undefined;
      i++;
      continue;
    }

    // Header line — also the fallback for everything else (query after a
    // header, grammar error recovery): a line that doesn't split on `:`
    // is preserved as a raw row so the user still sees it.
    const row = parseHeaderLine(trimmed, true, pendingDescription);
    if (row) {
      headers.push(row);
      sawHeader = true;
    } else {
      headers.push({
        key: "",
        value: raw,
        enabled: true,
        description: pendingDescription,
      });
    }
    pendingDescription = undefined;
    i++;
  }

  const bodyLines = lines.slice(i);
  // Drop trailing blank lines (idempotency: the stringifier never emits
  // trailing blank lines).
  while (bodyLines.length > 0 && bodyLines[bodyLines.length - 1] === "") {
    bodyLines.pop();
  }
  const bodyText = bodyLines.join("\n");

  return { method, url, params, headers, body: bodyText };
}

interface ParsedFirstLine {
  method: HttpMethod;
  url: string;
  inlineQuery: string;
}

function parseFirstLine(line: string): ParsedFirstLine | null {
  const m = line.match(/^([A-Z]+)\s+(\S.*)$/);
  if (!m) return null;
  const method = m[1] as HttpMethod;
  if (!HTTP_METHODS.has(method)) return null;
  const rest = m[2].trim();
  const qIdx = rest.indexOf("?");
  if (qIdx === -1) {
    return { method, url: rest, inlineQuery: "" };
  }
  return {
    method,
    url: rest.slice(0, qIdx),
    inlineQuery: rest.slice(qIdx + 1),
  };
}

function parseQuerySegment(
  seg: string,
  enabled: boolean,
  description: string | undefined,
): HttpKVRow | null {
  if (seg.length === 0) return null;
  const eq = seg.indexOf("=");
  let key: string;
  let value: string;
  if (eq === -1) {
    key = seg;
    value = "";
  } else {
    key = seg.slice(0, eq);
    value = seg.slice(eq + 1);
  }
  if (key.length === 0) return null;
  return description !== undefined
    ? { key, value, enabled, description }
    : { key, value, enabled };
}

function parseHeaderLine(
  line: string,
  enabled: boolean,
  description: string | undefined,
): HttpKVRow | null {
  const colonIdx = line.indexOf(":");
  if (colonIdx <= 0) return null;
  const key = line.slice(0, colonIdx).trim();
  const value = line.slice(colonIdx + 1).trim();
  if (key.length === 0) return null;
  return description !== undefined
    ? { key, value, enabled, description }
    : { key, value, enabled };
}

// ─────────────────────── Body emission ───────────────────────

/**
 * Emit canonical HTTP message body. Idempotent reformatter.
 *
 * Layout rules:
 * - First line: `METHOD URL[?inline_query]`. Inline query only if all params
 *   are enabled, none have descriptions, and the resulting line stays under
 *   ~80 characters. Otherwise each param is emitted on its own continuation
 *   line.
 * - Continuation lines: first param `?key=value`, rest `&key=value`.
 * - Disabled params/headers are emitted with a `# ` prefix.
 * - Descriptions are emitted as `# desc: <text>` on the line above.
 * - One blank line separates headers from body (omitted if body is empty AND
 *   there are no headers; otherwise always emitted when body is non-empty).
 * - Trailing whitespace stripped; output ends without trailing newline (caller
 *   adds the close fence).
 */
export function stringifyHttpMessageBody(parsed: HttpMessageParsed): string {
  const { method, url, params, headers, body } = parsed;
  const out: string[] = [];

  const inline = canInlineQuery(method, url, params);
  if (inline) {
    if (params.length > 0) {
      const q = params.map((p) => formatParam(p)).join("&");
      out.push(`${method} ${url}?${q}`);
    } else {
      out.push(`${method} ${url}`);
    }
  } else {
    out.push(`${method} ${url}`);
    let isFirst = true;
    for (const p of params) {
      if (p.description) {
        const prefix = p.enabled ? "" : "# ";
        out.push(`${prefix}${DESC_PREFIX}${p.description}`);
      }
      const seg = formatParam(p);
      const lead = isFirst ? "?" : "&";
      isFirst = false;
      const prefix = p.enabled ? "" : "# ";
      out.push(`${prefix}${lead}${seg}`);
    }
  }

  for (const h of headers) {
    if (h.description) {
      const prefix = h.enabled ? "" : "# ";
      out.push(`${prefix}${DESC_PREFIX}${h.description}`);
    }
    if (h.key.length === 0) {
      // raw / unparsed header preserved as-is
      out.push(h.value);
    } else {
      const prefix = h.enabled ? "" : "# ";
      out.push(`${prefix}${h.key}: ${h.value}`);
    }
  }

  if (body.length > 0) {
    out.push("");
    out.push(body);
  }

  return out.join("\n");
}

function canInlineQuery(
  method: HttpMethod,
  url: string,
  params: HttpKVRow[],
): boolean {
  if (params.length === 0) return true;
  // Force continuation if any disabled or any has description.
  if (params.some((p) => !p.enabled || p.description !== undefined))
    return false;
  const inline = params.map((p) => formatParam(p)).join("&");
  const total = method.length + 1 + url.length + 1 + inline.length;
  return total <= URL_INLINE_LIMIT;
}

function formatParam(p: HttpKVRow): string {
  if (p.value === "") return p.key;
  return `${p.key}=${p.value}`;
}
