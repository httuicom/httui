/**
 * Shared types and constants for the HTTP fenced-block UI.
 *
 * Lives next to HttpFencedPanel.tsx so the sub-components extracted from it
 * (HttpToolbar, HttpStatusBar, etc.) can import without pulling the whole
 * panel module.
 */

import type { HttpMethod } from "@/lib/blocks/http-fence";

// Canonical union lives in blocks/execution-state; re-exported so the
// HTTP sub-components keep importing it unchanged from "./shared".
export type { ExecutionState } from "@/components/blocks/execution-state";

export type SendAsFormat = "curl" | "fetch" | "python" | "httpie" | "http-file";

export const METHOD_COLORS: Record<HttpMethod, string> = {
  GET: "green.500",
  POST: "blue.500",
  PUT: "orange.500",
  PATCH: "yellow.500",
  DELETE: "red.500",
  HEAD: "purple.500",
  OPTIONS: "gray.500",
};

export const MUTATION_METHODS: ReadonlySet<HttpMethod> = new Set([
  "POST",
  "PUT",
  "PATCH",
  "DELETE",
]);

// ─────────────────────── Display helpers ───────────────────────

/** Pick a status-dot color from an HTTP status code (or gray when missing). */
export function statusDotColor(code: number | null | undefined): string {
  if (!code) return "gray.400";
  if (code >= 200 && code < 300) return "green.500";
  if (code >= 300 && code < 400) return "blue.500";
  if (code >= 400 && code < 500) return "orange.500";
  if (code >= 500) return "red.500";
  return "gray.400";
}

/** Format a byte count as `123 B`, `1.2 KB`, or `1.23 MB`. */
export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(2)} MB`;
}

/**
 * Convert an HTTP response body (string, JSON, or `{encoding:"base64"}` for
 * binary) into a plain text representation for raw/pretty display.
 */
export function bodyAsText(body: unknown): string {
  if (body === null || body === undefined) return "";
  if (typeof body === "string") return body;
  if (
    typeof body === "object" &&
    body !== null &&
    "encoding" in body &&
    (body as { encoding: string }).encoding === "base64"
  ) {
    return "[binary content — base64 encoded]";
  }
  try {
    return JSON.stringify(body, null, 2);
  } catch {
    return String(body);
  }
}

/** Round a `Date` into a "ran X ago" string ("just now" / "Xs ago" / etc). */
export function relativeTimeAgo(date: Date | null): string | null {
  if (!date) return null;
  const seconds = Math.round((Date.now() - date.getTime()) / 1000);
  if (seconds < 5) return "just now";
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.round(hours / 24);
  return `${days}d ago`;
}
