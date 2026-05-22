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
      // IPC failure or non-git vault — consumer shows `?` initials fallback.
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
