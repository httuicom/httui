// Polls the disk mtime of a vault note via the `get_file_mtime` Tauri
// command. Idles until both `vaultPath` and `filePath` are set;
// re-polls on window focus and exposes a `refresh()` for save-driven
// callers. Returns the mtime in epoch milliseconds (or `null` while
// unavailable / before the first poll completes).
//
// Carry-over from feeds the editor toolbar's
// "edited Xm ago" label without standing up a continuous timer the
// way the git-status poll does. The toolbar's relative-time renderer
// re-formats independently as the wall clock advances.
//
// Backed by `httui_core::vault_config::merge::mtime_or_none` so a
// renamed / deleted file naturally returns `null`.

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
      // Same posture as `useGitStatus`: swallow transient errors
      // (file may be mid-rename / mid-write); next focus / save
      // refresh recovers.
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
