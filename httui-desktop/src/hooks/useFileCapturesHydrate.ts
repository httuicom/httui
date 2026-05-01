// Epic 46 Story 03 — read-on-open captures hydration.
//
// On mount (or path change), reads `.httui/captures/<file>.json`
// via `readCapturesCache` and pipes the JSON through
// `useCaptureStore.loadFromCacheJson` so any persisted captures
// become available to subsequent block reference resolution
// without waiting for a re-run.
//
// Complementary to `useFileCapturesPersistence` (99cd74b) which
// handles the toggle ↔ write/delete side. This hook owns the
// read-on-mount side. Idle when either path is null. Errors are
// swallowed — the cache is best-effort, a missing/corrupt file
// shouldn't make the editor surface an error.

import { useEffect, useRef } from "react";

import { readCapturesCache } from "@/lib/tauri/captures-cache";
import { useCaptureStore } from "@/stores/captureStore";

export function useFileCapturesHydrate(
  vaultPath: string | null,
  filePath: string | null,
): void {
  const cancelledRef = useRef(false);

  useEffect(() => {
    cancelledRef.current = false;
    if (!vaultPath || !filePath) {
      return () => {
        cancelledRef.current = true;
      };
    }
    void (async () => {
      try {
        const json = await readCapturesCache(vaultPath, filePath);
        if (cancelledRef.current) return;
        if (json) {
          useCaptureStore.getState().loadFromCacheJson(filePath, json);
        }
      } catch {
        // Best-effort: cache miss / corruption shouldn't surface.
      }
    })();
    return () => {
      cancelledRef.current = true;
    };
  }, [vaultPath, filePath]);
}
