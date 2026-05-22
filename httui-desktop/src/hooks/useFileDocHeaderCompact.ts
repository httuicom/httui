import { useCallback, useEffect, useRef, useState } from "react";

import { getFileSettings, setFileDocheaderCompact } from "@/lib/tauri/files";

export interface UseFileDocHeaderCompactResult {
  compact: boolean;
  loaded: boolean;
  setCompact: (next: boolean) => Promise<void>;
}

/** Hook the per-file `docheader_compact` flag. */
export function useFileDocHeaderCompact(
  vaultPath: string | null,
  filePath: string | null,
): UseFileDocHeaderCompactResult {
  const [compact, setLocal] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const cancelledRef = useRef(false);

  const fetchOnce = useCallback(async () => {
    if (!vaultPath || !filePath) {
      setLocal(false);
      setLoaded(false);
      return;
    }
    try {
      const settings = await getFileSettings(vaultPath, filePath);
      if (cancelledRef.current) return;
      // Rust skips the field at its default; coerce undefined → false.
      setLocal(Boolean(settings.docheader_compact));
      setLoaded(true);
    } catch {
      if (cancelledRef.current) return;
      setLocal(false);
      setLoaded(false);
    }
  }, [vaultPath, filePath]);

  useEffect(() => {
    cancelledRef.current = false;
    void fetchOnce();
    return () => {
      cancelledRef.current = true;
    };
  }, [fetchOnce]);

  const setCompact = useCallback(
    async (next: boolean) => {
      if (!vaultPath || !filePath) return;
      const prev = compact;
      setLocal(next);
      try {
        await setFileDocheaderCompact(vaultPath, filePath, next);
      } catch (e) {
        setLocal(prev); // Roll back to on-disk truth on failure.
        throw e;
      }
    },
    [vaultPath, filePath, compact],
  );

  return { compact, loaded, setCompact };
}
