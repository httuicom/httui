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
