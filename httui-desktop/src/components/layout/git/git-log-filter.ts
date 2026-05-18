// pure log-filter helpers.
//
// Author filter is in-memory: the consumer keeps the full
// `CommitInfo[]` and we just filter the array. Path filter goes
// through the backend (`git_log(vault, limit, path)`) — there's no
// in-memory equivalent here because `CommitInfo` doesn't carry the
// touched paths. The `parsePathFilter` helper normalizes user input
// (trims, drops trailing slashes, collapses internal whitespace).

import type { CommitInfo } from "@/lib/tauri/git";

/**
 * Case-insensitive substring match against `author_name` or
 * `author_email`. An empty / whitespace-only query returns the
 * input unchanged (no filtering).
 */
export function filterCommitsByAuthor(
  commits: ReadonlyArray<CommitInfo>,
  query: string,
): CommitInfo[] {
  const q = query.trim().toLowerCase();
  if (q.length === 0) return commits.slice();
  return commits.filter((c) => {
    const name = c.author_name.toLowerCase();
    const email = c.author_email.toLowerCase();
    return name.includes(q) || email.includes(q);
  });
}

/**
 * Normalize a path-filter query for handing to `git_log`. Returns
 * `null` for empty / whitespace input so the consumer can call
 * `git_log(vault, limit)` (no path filter) instead of
 * `git_log(vault, limit, "")`.
 */
export function parsePathFilter(query: string): string | null {
  const trimmed = query.trim();
  if (trimmed.length === 0) return null;
  // Drop trailing slashes — git treats "src" and "src/" the same and
  // the trailing-slash variant breaks pathspec interpretation in some
  // edge cases.
  return trimmed.replace(/\/+$/u, "");
}

export type LogFilterMode = "author" | "path";

export interface LogFilterState {
  mode: LogFilterMode;
  query: string;
}

/** Central place to toggle filter mode while preserving the query. */
export function toggleFilterMode(state: LogFilterState): LogFilterState {
  return {
    mode: state.mode === "author" ? "path" : "author",
    query: state.query,
  };
}
