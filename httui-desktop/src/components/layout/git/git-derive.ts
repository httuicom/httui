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

/** Tally changes by kind. Renamed/copied fold into `modified`. */
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
  // XY porcelain v2: prefer worktree char when set, else staged char.
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

/** Two-letter author initials from `author_name`. Falls back to `?`. */
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
