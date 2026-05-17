// Reads + writes the per-file `auto_capture` flag persisted in
// `.httui/workspace.toml` via the `get_file_settings` /
// `set_file_auto_capture` Tauri commands. Carry-over from Epic 39
// Story 03 — backs the editor toolbar's auto-capture toggle.
//
// Idle when either path is null. Optimistic update on toggle: the
// frontend reflects the new value immediately while the Tauri call
// is in flight, and rolls back to the on-disk value if the persist
// fails. The store mutates the **base** file (not `.local`) per
// audit-003.

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
      // No-op on transient errors; same posture as `useFileMtime`.
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
        // Roll back so the UI reflects on-disk truth.
        setLocal(prev);
        throw e;
      }
    },
    [vaultPath, filePath, autoCapture],
  );

  return { autoCapture, loaded, setAutoCapture };
}
