import { useCallback } from "react";

import {
  deleteCapturesCache,
  writeCapturesCache,
} from "@/lib/tauri/captures-cache";
import { useCaptureStore } from "@/stores/captureStore";

import {
  useFileAutoCapture,
  type UseFileAutoCaptureResult,
} from "./useFileAutoCapture";

export type UseFileCapturesPersistenceResult = UseFileAutoCaptureResult;

export function useFileCapturesPersistence(
  vaultPath: string | null,
  filePath: string | null,
): UseFileCapturesPersistenceResult {
  const inner = useFileAutoCapture(vaultPath, filePath);
  const { setAutoCapture: setInner } = inner;
  const setAutoCapture = useCallback(
    async (next: boolean) => {
      // Inner persist runs first; on failure it throws and skips the cache I/O below.
      await setInner(next);
      if (!vaultPath || !filePath) return;
      try {
        if (next) {
          const json = useCaptureStore.getState().dumpForCacheJson(filePath);
          if (json !== null) {
            await writeCapturesCache(vaultPath, filePath, json);
          }
        } else {
          await deleteCapturesCache(vaultPath, filePath);
        }
      } catch {
        // Best-effort: cache failure doesn't affect the committed auto_capture flag.
      }
    },
    [setInner, vaultPath, filePath],
  );
  return { ...inner, setAutoCapture };
}
