// Epic 48 Story 01 — pure derivations for the Git panel UI.
//
// Kept framework-free so the same helpers feed `GitStatusHeader`,
// `GitFileList`, and `GitLogList` plus their tests. Consumers (the
// future panel container) call `gitStatus`/`gitLog` and pass results
// straight to the presentational components.

import type { CommitInfo, GitFileChange, GitStatus } from "@/lib/tauri/git";

export interface FilePartition {
  staged: GitFileChange[];
  unstaged: GitFileChange[];
  untracked: GitFileChange[];
}

export function partitionFileChanges(
  changed: ReadonlyArray<GitFileChange>,
): FilePartition {
  const staged: GitFileChange[] = [];
  const unstaged: GitFileChange[] = [];
  const untracked: GitFileChange[] = [];
  for (const f of changed) {
    if (f.untracked) {
      untracked.push(f);
    } else if (f.staged) {
      staged.push(f);
    } else {
      unstaged.push(f);
    }
  }
  return { staged, unstaged, untracked };
}

export interface BranchSummary {
  /** Display label — branch name or "(detached)". */
  label: string;
  upstream: string | null;
  ahead: number;
  behind: number;
  /** `true` only when ahead == behind == 0 AND upstream is set. */
  inSync: boolean;
  /** `true` when there is no upstream branch — UI shows a hint. */
  noUpstream: boolean;
}

export function summarizeBranch(status: GitStatus): BranchSummary {
  return {
    label: status.branch ?? "(detached)",
    upstream: status.upstream,
    ahead: status.ahead,
    behind: status.behind,
    inSync: !!status.upstream && status.ahead === 0 && status.behind === 0,
    noUpstream: status.upstream === null,
  };
}

export interface ChangeCounts {
  modified: number;
  added: number;
  deleted: number;
  untracked: number;
  conflicted: number;
}

/** Tally the working-tree changes by kind for the pane-tab metrics
 *  strip (V10.1 cenário 6). Renamed/copied/other fold into
 *  `modified` — the strip wants a dense at-a-glance count, not the
 *  full porcelain taxonomy. */
export function summarizeChangeCounts(
  changed: ReadonlyArray<GitFileChange>,
): ChangeCounts {
  const counts: ChangeCounts = {
    modified: 0,
    added: 0,
    deleted: 0,
    untracked: 0,
    conflicted: 0,
  };
  for (const f of changed) {
    switch (labelFileStatus(f)) {
      case "untracked":
        counts.untracked += 1;
        break;
      case "added":
        counts.added += 1;
        break;
      case "deleted":
        counts.deleted += 1;
        break;
      case "conflicted":
        counts.conflicted += 1;
        break;
      default:
        counts.modified += 1;
    }
  }
  return counts;
}

/** Map a 2-char porcelain v2 XY status to a single human-readable token. */
export function labelFileStatus(file: GitFileChange): string {
  if (file.untracked) return "untracked";
  // status is the XY field from `git status --porcelain=v2`.
  // First char = staged, second = worktree. We collapse to the most
  // informative letter, preferring worktree when present.
  const xy = file.status ?? "..";
  const staged = xy.charAt(0);
  const worktree = xy.charAt(1);
  const code = worktree !== "." ? worktree : staged;
  switch (code) {
    case "M":
      return "modified";
    case "A":
      return "added";
    case "D":
      return "deleted";
    case "R":
      return "renamed";
    case "C":
      return "copied";
    case "U":
      return "conflicted";
    default:
      return "changed";
  }
}

/**
 * Best-effort relative time formatter. Accepts either a Unix-seconds
 * timestamp (CommitInfo) or an ISO string (we round-trip through
 * `Date.parse`). Falls back to the raw string when parsing fails.
 */
export function relativeTime(
  input: number | string,
  now: number = Date.now(),
): string {
  let ms: number;
  if (typeof input === "number") {
    ms = input * 1000;
  } else {
    const t = Date.parse(input);
    if (Number.isNaN(t)) return input;
    ms = t;
  }
  const diffSec = Math.max(0, Math.floor((now - ms) / 1000));
  if (diffSec < 60) return `${diffSec}s ago`;
  if (diffSec < 3600) return `${Math.floor(diffSec / 60)}m ago`;
  if (diffSec < 86400) return `${Math.floor(diffSec / 3600)}h ago`;
  return `${Math.floor(diffSec / 86400)}d ago`;
}

/**
 * Two-letter author initials derived from `author_name`. Used in the
 * log row to keep it compact (the spec requests "author initials").
 * Falls back to `?` when no usable letters can be extracted.
 */
export function authorInitials(commit: CommitInfo): string {
  const name = commit.author_name?.trim() ?? "";
  if (!name) return "?";
  const parts = name.split(/\s+/u).filter((p) => p.length > 0);
  if (parts.length === 0) return "?";
  if (parts.length === 1) {
    return parts[0].slice(0, 2).toUpperCase();
  }
  return (parts[0]![0]! + parts[parts.length - 1]![0]!).toUpperCase();
}
