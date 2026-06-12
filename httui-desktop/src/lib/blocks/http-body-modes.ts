/**
 * Body-mode helpers for the HTTP block: the Content-Type ↔ pill mapping,
 * the simplified multipart KV body, and the single-line binary body.
 *
 * The pill is a *view* over the `Content-Type` header — selecting a mode
 * rewrites only that header value (or removes it for `none`); the body
 * itself is never touched.
 */

import type { HttpKVRow, HttpMessageParsed } from "./http-message";

/** Body mode pill values. `none` means no `Content-Type` header is set. */
export type HttpBodyMode =
  | "none"
  | "json"
  | "xml"
  | "text"
  | "form-urlencoded"
  | "multipart"
  | "binary";

const BODY_MODE_TO_CT: Record<Exclude<HttpBodyMode, "none">, string> = {
  json: "application/json",
  xml: "application/xml",
  text: "text/plain",
  "form-urlencoded": "application/x-www-form-urlencoded",
  multipart: "multipart/form-data",
  binary: "application/octet-stream",
};

const TEXTUAL_MODES: ReadonlySet<HttpBodyMode> = new Set([
  "json",
  "xml",
  "text",
]);

const STRUCTURED_MODES: ReadonlySet<HttpBodyMode> = new Set([
  "form-urlencoded",
  "multipart",
  "binary",
]);

const CONTENT_TYPE_HEADER = "Content-Type";

function findContentTypeIndex(headers: HttpKVRow[]): number {
  return headers.findIndex((h) => h.key.toLowerCase() === "content-type");
}

/**
 * Map a `Content-Type` value (without parameters like `; charset=utf-8`) to a
 * pill mode. Unknown types fall back to `text` so the user can still
 * reformat; absence of the header yields `none`.
 */
export function deriveBodyMode(headers: HttpKVRow[]): HttpBodyMode {
  const idx = findContentTypeIndex(headers);
  if (idx === -1) return "none";
  const row = headers[idx];
  if (!row.enabled) return "none";
  // Strip parameters (`; charset=utf-8`, `; boundary=...`) and lowercase.
  const mime = row.value.split(";")[0].trim().toLowerCase();
  if (mime === "") return "none";
  if (mime === "application/json" || mime.endsWith("+json")) return "json";
  if (
    mime === "application/xml" ||
    mime === "text/xml" ||
    mime.endsWith("+xml")
  )
    return "xml";
  if (mime === "text/plain") return "text";
  if (mime === "application/x-www-form-urlencoded") return "form-urlencoded";
  if (mime.startsWith("multipart/")) return "multipart";
  if (
    mime === "application/octet-stream" ||
    mime.startsWith("image/") ||
    mime.startsWith("audio/") ||
    mime.startsWith("video/") ||
    mime === "application/pdf"
  )
    return "binary";
  if (mime.startsWith("text/")) return "text";
  // Default for unknown types: keep as text so the user can edit cleanly.
  return "text";
}

/**
 * Set the `Content-Type` header so it matches the desired body mode. Idempotent
 * and surgical — body, params, URL and other headers are never touched.
 *
 * - `none` removes any existing `Content-Type` header (case-insensitive).
 * - Any other mode replaces the value of an existing header (preserving
 *   description/disabled flag) or appends a new enabled header at the end of
 *   the headers list.
 */
export function setContentTypeForMode(
  parsed: HttpMessageParsed,
  next: HttpBodyMode,
): HttpMessageParsed {
  const idx = findContentTypeIndex(parsed.headers);
  const headers = parsed.headers.slice();

  if (next === "none") {
    if (idx === -1) return parsed;
    headers.splice(idx, 1);
    return { ...parsed, headers };
  }

  const value = BODY_MODE_TO_CT[next];
  if (idx === -1) {
    headers.push({ key: CONTENT_TYPE_HEADER, value, enabled: true });
  } else {
    const prev = headers[idx];
    if (prev.value === value && prev.enabled) return parsed;
    headers[idx] = { ...prev, value, enabled: true };
  }
  return { ...parsed, headers };
}

/**
 * Decide whether switching from `prev` to `next` is "compatible" with the
 * current body. Used to surface a warning toast — the switch is *never*
 * blocked, the user just gets a heads-up that the body shape no longer
 * matches the declared `Content-Type`.
 *
 * Compatibility rules (intentionally loose):
 * - If the body is empty → always compatible (nothing to misinterpret).
 * - Same mode → compatible.
 * - Toggling between textual modes (json/xml/text) → compatible (the user
 *   is just changing the declared subtype; payload may still need editing
 *   but it's not a structural mismatch).
 * - Going from a textual body to a structured mode (form-urlencoded /
 *   multipart / binary) → **incompatible** (current text won't parse).
 * - Going from `none` → anything is compatible (we're only adding a
 *   declaration).
 */
export function isCompatibleSwitch(
  prev: HttpBodyMode,
  next: HttpBodyMode,
  body: string,
): boolean {
  if (prev === next) return true;
  if (body.trim().length === 0) return true;
  if (prev === "none") return true;
  if (TEXTUAL_MODES.has(prev) && TEXTUAL_MODES.has(next)) return true;
  if (TEXTUAL_MODES.has(prev) && STRUCTURED_MODES.has(next)) return false;
  // Structured → textual or structured → structured: textual content is
  // unlikely to live there anyway; allow with no warning.
  return true;
}

// ─────────────────────── Multipart body ───────────────────────

export type MultipartPartKind = "text" | "file";

export interface MultipartPart {
  kind: MultipartPartKind;
  name: string;
  /** Text payload OR absolute file path (when `kind === "file"`). */
  value: string;
  /** File mode only — defaults to basename of `value`. */
  filename?: string;
  /** File mode only — auto-inferred from extension; user can override. */
  contentType?: string;
  enabled: boolean;
  description?: string;
}

const DESC_PREFIX = "# desc: ";
const FILE_PREFIX = "< ";

/**
 * Best-effort MIME inference from filename. Used when a `file` part doesn't
 * carry an explicit `Content-Type` header. Falls back to `application/octet-stream`.
 */
export function inferContentType(filename: string): string {
  const idx = filename.lastIndexOf(".");
  const ext = idx === -1 ? "" : filename.slice(idx + 1).toLowerCase();
  switch (ext) {
    case "json":
      return "application/json";
    case "xml":
      return "application/xml";
    case "html":
    case "htm":
      return "text/html";
    case "css":
      return "text/css";
    case "js":
      return "application/javascript";
    case "txt":
    case "log":
    case "md":
      return "text/plain";
    case "csv":
      return "text/csv";
    case "png":
      return "image/png";
    case "jpg":
    case "jpeg":
      return "image/jpeg";
    case "gif":
      return "image/gif";
    case "webp":
      return "image/webp";
    case "svg":
      return "image/svg+xml";
    case "pdf":
      return "application/pdf";
    case "zip":
      return "application/zip";
    case "mp3":
      return "audio/mpeg";
    case "mp4":
      return "video/mp4";
    case "wav":
      return "audio/wav";
    case "webm":
      return "video/webm";
    default:
      return "application/octet-stream";
  }
}

function basename(p: string): string {
  // Cross-platform-ish: split on both / and \ and take the last segment.
  const parts = p.split(/[\\/]/).filter((x) => x.length > 0);
  return parts.length > 0 ? parts[parts.length - 1] : p;
}

/**
 * Parse the simplified KV multipart body. Each line is one part:
 *
 *   name=value           ← text part
 *   name=< /path/to/file ← file part
 *   # name=value         ← disabled (text or file)
 *   # desc: …            ← description for the part on the next line
 *
 * Empty lines and free-form `#…` comments (anything that isn't `# name=…`
 * or `# desc:`) are ignored. Returns an empty array on a body that
 * doesn't look like multipart KV — caller falls back to treating the
 * body as raw text.
 */
export function parseMultipartBody(body: string): MultipartPart[] {
  if (body.trim().length === 0) return [];
  const lines = body.split("\n");
  const parts: MultipartPart[] = [];
  let pendingDescription: string | undefined;

  for (const raw of lines) {
    const trimmed = raw.trim();
    if (trimmed === "") {
      continue;
    }

    // Description for the next part line. Reset on consume below.
    if (trimmed.startsWith(DESC_PREFIX)) {
      pendingDescription = trimmed.slice(DESC_PREFIX.length);
      continue;
    }

    // `# ` or bare `#` prefix → disabled. Strip and re-parse the rest;
    // anything that isn't a `name=…` after stripping is a free-form
    // comment and gets dropped (with the pending description).
    let enabled = true;
    let body = trimmed;
    if (trimmed.startsWith("# ")) {
      enabled = false;
      body = trimmed.slice(2);
    } else if (trimmed === "#") {
      // bare `#` — drop the pending description and continue.
      pendingDescription = undefined;
      continue;
    } else if (trimmed.startsWith("#")) {
      // `#anything` without a space → free-form comment.
      pendingDescription = undefined;
      continue;
    }

    const eq = body.indexOf("=");
    if (eq <= 0) {
      pendingDescription = undefined;
      continue;
    }
    const name = body.slice(0, eq).trim();
    const rawValue = body.slice(eq + 1);
    if (name.length === 0) {
      pendingDescription = undefined;
      continue;
    }

    // File part if value starts with `< ` (file ref). We do NOT trim the
    // value first — leading whitespace in a text payload would be lost.
    if (rawValue.startsWith(FILE_PREFIX)) {
      const path = rawValue.slice(FILE_PREFIX.length).trim();
      const filename = basename(path);
      parts.push({
        kind: "file",
        name,
        value: path,
        filename,
        contentType: inferContentType(filename),
        enabled,
        ...(pendingDescription !== undefined && {
          description: pendingDescription,
        }),
      });
    } else {
      parts.push({
        kind: "text",
        name,
        value: rawValue,
        enabled,
        ...(pendingDescription !== undefined && {
          description: pendingDescription,
        }),
      });
    }
    pendingDescription = undefined;
  }

  return parts;
}

/**
 * Serialize multipart parts to the simplified KV format. Idempotent: feeding
 * the output back into `parseMultipartBody` and re-stringifying yields the
 * same text.
 *
 * No transport boundary in the textual form — reqwest generates the real
 * boundary on the wire when it sends the request.
 */
export function stringifyMultipartBody(parts: MultipartPart[]): {
  body: string;
} {
  const lines: string[] = [];
  for (const part of parts) {
    const prefix = part.enabled ? "" : "# ";
    if (part.description !== undefined) {
      lines.push(`${DESC_PREFIX}${part.description}`);
    }
    if (part.kind === "file") {
      lines.push(`${prefix}${part.name}=${FILE_PREFIX}${part.value}`);
    } else {
      lines.push(`${prefix}${part.name}=${part.value}`);
    }
  }
  return { body: lines.join("\n") };
}

// ─────────────────────── Binary body ───────────────────────

/**
 * Detect a body that is exactly a single `< /path/to/file` line (binary mode).
 * Returns `{ path }` when matched, `null` otherwise. Whitespace around the
 * line is tolerated so the user can hit Enter without losing the binding.
 */
export function isBinaryFileBody(body: string): { path: string } | null {
  const trimmed = body.trim();
  if (!trimmed.startsWith(FILE_PREFIX)) return null;
  const path = trimmed.slice(FILE_PREFIX.length).trim();
  if (path.length === 0) return null;
  // Must be a single line — if there's a newline anywhere with content, it's
  // not a clean binary body (likely the user is mixing text in).
  const lines = trimmed.split("\n").filter((l) => l.trim().length > 0);
  if (lines.length !== 1) return null;
  return { path };
}

/** Build the canonical single-line body for a binary upload. */
export function buildBinaryFileBody(path: string): string {
  return `${FILE_PREFIX}${path}`;
}
