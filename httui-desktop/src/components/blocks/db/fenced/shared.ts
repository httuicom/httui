export type ExecutionState =
  | "idle"
  | "running"
  | "success"
  | "error"
  | "cancelled";

// Canonical formatter lives in lib/format; re-exported so the DB
// sub-components keep importing it unchanged from "./shared".
export { formatElapsed } from "@/lib/format/time";

/** Human-friendly relative timestamp: "just now", "3m ago", "2h ago", "1d ago". Falls back to ISO date after 7 days. */
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
