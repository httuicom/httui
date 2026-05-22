import { useCallback, useEffect, useRef, useState } from "react";

import { getFileMtime } from "@/lib/tauri/files";

export interface UseFileMtimeResult {
  mtime: number | null;
  refresh: () => void;
}

/** Subscribe to a note's filesystem mtime. `vaultPath` and
 * `filePath` may be `null` to disable the poll. */
export function useFileMtime(
  vaultPath: string | null,
  filePath: string | null,
): UseFileMtimeResult {
  const [mtime, setMtime] = useState<number | null>(null);
  const cancelledRef = useRef(false);

  const fetchOnce = useCallback(async () => {
    if (!vaultPath || !filePath) {
      setMtime(null);
      return;
    }
    try {
      const next = await getFileMtime(vaultPath, filePath);
      if (cancelledRef.current) return;
      setMtime(next);
    } catch {
      if (cancelledRef.current) return;
      // Swallow transient errors (mid-rename/mid-write); next focus or save recovers.
      setMtime(null);
    }
  }, [vaultPath, filePath]);

  useEffect(() => {
    cancelledRef.current = false;
    void fetchOnce();
    const onFocus = () => {
      void fetchOnce();
    };
    window.addEventListener("focus", onFocus);
    return () => {
      cancelledRef.current = true;
      window.removeEventListener("focus", onFocus);
    };
  }, [fetchOnce]);

  const refresh = useCallback(() => {
    void fetchOnce();
  }, [fetchOnce]);

  return { mtime, refresh };
}
