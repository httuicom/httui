/**
 * Shared types and helpers for the DB fenced-block UI.
 *
 * Lives next to DbFencedPanel.tsx so the sub-components extracted from it
 * (DbToolbar, DbStatusBar, etc.) can import without pulling the whole
 * panel module.
 */

// Canonical union lives in blocks/execution-state; re-exported so the
// DB sub-components keep importing it unchanged from "./shared".
export type { ExecutionState } from "@/components/blocks/execution-state";

// Canonical formatter lives in lib/format; re-exported so the DB
// sub-components keep importing it unchanged from "./shared".
export { formatElapsed } from "@/lib/format/time";

/**
 * Human-friendly relative timestamp: "just now", "3m ago", "2h ago", "1d ago".
 * Used to render the "last run" hint in the status bar without a dependency
 * on a date library. Capped at days — anything older is suspiciously stale
 * and we render the ISO date instead.
 */
export function formatRelativeTime(from: number, now: number): string {
  const delta = Math.max(0, Math.floor((now - from) / 1000)); // seconds
  if (delta < 5) return "just now";
  if (delta < 60) return `${delta}s ago`;
  const minutes = Math.floor(delta / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d ago`;
  return new Date(from).toISOString().slice(0, 10);
}

/** Lightweight type guard used by result-tab views. */
export function isPlainObject(v: unknown): v is Record<string, unknown> {
  return v !== null && typeof v === "object" && !Array.isArray(v);
}
