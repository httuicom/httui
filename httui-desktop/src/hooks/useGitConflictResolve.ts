// Epic 48 Story 06 — accept-side conflict resolver hook.
//
// Wraps the two-step accept-and-mark-resolved chain shipped in
// commits 97288f5 (Rust `git_checkout_conflict_path`) + e8c4dad
// (Tauri + TS wrapper). The `<GitConflictBanner>` component
// (ccb7ea0) emits `onAcceptYours(path)` / `onAcceptTheirs(path)`;
// this hook turns those callbacks into a single async invocation
// that:
//
// 1. `gitCheckoutConflictPath(vault, path, side)` — replaces the
//    working-tree file with the chosen side
// 2. `stagePath(vault, path)` — marks the conflict resolved by
//    re-adding the path to the index
//
// `busy` is the lifecycle flag the banner uses to disable buttons
// during the operation. `error` carries the verbatim git stderr so
// the consumer toast can surface the actual reason.

import { useCallback, useState } from "react";

import {
  gitCheckoutConflictPath,
  stagePath,
  type ConflictSide,
} from "@/lib/tauri/git";

export interface UseGitConflictResolveResult {
  busy: boolean;
  error: string | null;
  acceptOurs: (path: string) => Promise<void>;
  acceptTheirs: (path: string) => Promise<void>;
  /** Reset the error state without firing another accept. */
  clearError: () => void;
}

export function useGitConflictResolve(
  vaultPath: string | null,
): UseGitConflictResolveResult {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const accept = useCallback(
    async (path: string, side: ConflictSide) => {
      if (!vaultPath || busy) return;
      setBusy(true);
      setError(null);
      try {
        await gitCheckoutConflictPath(vaultPath, path, side);
        await stagePath(vaultPath, path);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        setBusy(false);
      }
    },
    [vaultPath, busy],
  );

  const acceptOurs = useCallback(
    (path: string) => accept(path, "ours"),
    [accept],
  );
  const acceptTheirs = useCallback(
    (path: string) => accept(path, "theirs"),
    [accept],
  );
  const clearError = useCallback(() => setError(null), []);

  return { busy, error, acceptOurs, acceptTheirs, clearError };
}
