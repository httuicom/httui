// coverage:exclude file — pure invoke() wrappers + IPC types.
// See tech-debt.md "coverage opt-out" for the same rationale used
// in commands.ts.

import { invoke } from "@tauri-apps/api/core";

export interface GitFileChange {
  path: string;
  status: string;
  staged: boolean;
  untracked: boolean;
}

export interface GitStatus {
  branch: string | null;
  upstream: string | null;
  ahead: number;
  behind: number;
  changed: GitFileChange[];
  clean: boolean;
}

export interface CommitInfo {
  sha: string;
  short_sha: string;
  author_name: string;
  author_email: string;
  /** Author timestamp as Unix seconds. */
  timestamp: number;
  subject: string;
}

export interface BranchInfo {
  name: string;
  current: boolean;
  remote: boolean;
}

export function gitStatus(vaultPath: string): Promise<GitStatus> {
  return invoke("git_status_cmd", { vaultPath });
}

export function gitLog(
  vaultPath: string,
  limit: number,
  pathFilter?: string,
): Promise<CommitInfo[]> {
  return invoke("git_log_cmd", {
    vaultPath,
    limit,
    pathFilter: pathFilter ?? null,
  });
}

export function gitDiff(
  vaultPath: string,
  commitSha?: string,
): Promise<string> {
  return invoke("git_diff_cmd", { vaultPath, commitSha: commitSha ?? null });
}

export function gitBranchList(vaultPath: string): Promise<BranchInfo[]> {
  return invoke("git_branch_list_cmd", { vaultPath });
}

export interface Remote {
  name: string;
  url: string;
}

export function gitRemoteList(vaultPath: string): Promise<Remote[]> {
  return invoke("git_remote_list_cmd", { vaultPath });
}

export function gitCheckout(vaultPath: string, branch: string): Promise<void> {
  return invoke("git_checkout_cmd", { vaultPath, branch });
}

/** First-commit author of `path` (follows renames). Resolves to
 *  `null` when the path doesn't appear in history. Powers Epic 50
 *  Story 03's `<DocHeaderMetaStrip>` Author chip. */
export function gitFirstCommitAuthor(
  vaultPath: string,
  path: string,
): Promise<CommitInfo | null> {
  return invoke("git_first_commit_author_cmd", { vaultPath, path });
}

export function gitCheckoutB(
  vaultPath: string,
  newBranch: string,
): Promise<void> {
  return invoke("git_checkout_b_cmd", { vaultPath, newBranch });
}

export function stagePath(vaultPath: string, path: string): Promise<void> {
  return invoke("stage_path_cmd", { vaultPath, path });
}

export function unstagePath(vaultPath: string, path: string): Promise<void> {
  return invoke("unstage_path_cmd", { vaultPath, path });
}

export function gitCommit(
  vaultPath: string,
  message: string,
  amend: boolean,
): Promise<void> {
  return invoke("git_commit_cmd", { vaultPath, message, amend });
}

export function gitFetch(
  vaultPath: string,
  remote?: string,
): Promise<string> {
  return invoke("git_fetch_cmd", { vaultPath, remote: remote ?? null });
}

export function gitPull(
  vaultPath: string,
  remote?: string,
  branch?: string,
): Promise<string> {
  return invoke("git_pull_cmd", {
    vaultPath,
    remote: remote ?? null,
    branch: branch ?? null,
  });
}

export function gitPush(
  vaultPath: string,
  remote?: string,
  branch?: string,
): Promise<string> {
  return invoke("git_push_cmd", {
    vaultPath,
    remote: remote ?? null,
    branch: branch ?? null,
  });
}
