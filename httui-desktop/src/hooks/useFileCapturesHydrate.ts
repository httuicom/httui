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
