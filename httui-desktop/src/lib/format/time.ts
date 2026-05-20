/**
 * Shared time/duration formatters.
 *
 * NOTE: relative-time ("X ago") is intentionally NOT unified here. The
 * three call sites (`git/git-derive.ts`, `http/fenced/shared.ts`,
 * `db/fenced/shared.ts`) have genuinely divergent contracts — floor vs
 * round, with/without a "just now" bucket, 7d→ISO cap vs none, and
 * different input types (Date | epoch-ms | epoch-s | ISO). Collapsing
 * them would silently change rendered timestamps in git/http/db, so it
 * is a deliberate behavior-normalization decision, not a mechanical
 * dedup. See docs-llm/code-audit/01-duplication.md §3.
 */

/** Elapsed duration as `123ms` or `1.23s` (two-decimal seconds). */
export function formatElapsed(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(2)}s`;
}

/**
 * Compact elapsed duration: `12ms` / `1.2s` / `3m` depending on
 * magnitude (one-decimal seconds, minutes above 60s). Used by the
 * history list where space is tight.
 */
export function formatDurationCompact(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  return `${Math.round(ms / 60_000)}m`;
}
