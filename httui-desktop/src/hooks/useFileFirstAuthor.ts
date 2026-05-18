// Fetches the first-commit author of `filePath` once per
// (vaultPath, filePath) change. Powers the
// `<DocHeaderMetaStrip>` Author chip — the chip itself is already
// shipped, this is the data source.
//
// Idle when either path is null (matches `useFileMtime` /
// `useFileAutoCapture` posture). `null` author + `loaded: true` is
// the legitimate "this path doesn't appear in git history yet"
// signal — the consumer falls back to the `?` initials per the
// `<DocHeaderMetaStrip>` contract.

import { useCallback, useEffect, useRef, useState } from "react";

import { gitFirstCommitAuthor, type CommitInfo } from "@/lib/tauri/git";

export interface UseFileFirstAuthorResult {
  author: CommitInfo | null;
  loaded: boolean;
  error: string | null;
  refresh: () => void;
}

export function useFileFirstAuthor(
  vaultPath: string | null,
  filePath: string | null,
): UseFileFirstAuthorResult {
  const [author, setAuthor] = useState<CommitInfo | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const cancelledRef = useRef(false);

  const fetchOnce = useCallback(async () => {
    if (!vaultPath || !filePath) {
      setAuthor(null);
      setLoaded(false);
      setError(null);
      return;
    }
    try {
      const next = await gitFirstCommitAuthor(vaultPath, filePath);
      if (cancelledRef.current) return;
      setAuthor(next);
      setLoaded(true);
      setError(null);
    } catch (e) {
      if (cancelledRef.current) return;
      // IPC failure / non-git vault — surface to the consumer so
      // the chip can render its `?` fallback knowingly.
      setAuthor(null);
      setLoaded(false);
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [vaultPath, filePath]);

  useEffect(() => {
    cancelledRef.current = false;
    void fetchOnce();
    return () => {
      cancelledRef.current = true;
    };
  }, [fetchOnce]);

  const refresh = useCallback(() => {
    void fetchOnce();
  }, [fetchOnce]);

  return { author, loaded, error, refresh };
}
