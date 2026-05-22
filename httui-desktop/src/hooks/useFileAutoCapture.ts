import { useCallback, useEffect, useRef, useState } from "react";

import { getFileSettings, setFileAutoCapture } from "@/lib/tauri/files";

export interface UseFileAutoCaptureResult {
  autoCapture: boolean;
  loaded: boolean;
  setAutoCapture: (next: boolean) => Promise<void>;
}

/** Hook the per-file `auto_capture` flag. */
export function useFileAutoCapture(
  vaultPath: string | null,
  filePath: string | null,
): UseFileAutoCaptureResult {
  const [autoCapture, setLocal] = useState(false);
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
      setLocal(Boolean(settings.auto_capture));
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

  const setAutoCapture = useCallback(
    async (next: boolean) => {
      if (!vaultPath || !filePath) return;
      const prev = autoCapture;
      setLocal(next);
      try {
        await setFileAutoCapture(vaultPath, filePath, next);
      } catch (e) {
        setLocal(prev); // Roll back to on-disk truth on failure.
        throw e;
      }
    },
    [vaultPath, filePath, autoCapture],
  );

  return { autoCapture, loaded, setAutoCapture };
}
