// auto-capture toggle ↔ persistence wiring.
//
// Composes `useFileAutoCapture` (the per-file flag stored in
// `.httui/workspace.toml`) with the captures-cache file I/O at
// `.httui/captures/<file_relpath>.json`. When the user flips
// auto-capture ON, the in-memory captures map is filtered (secrets
// dropped, non-secrets kept) via `dumpForCacheJson` and written to
// disk; flipping OFF deletes the cache file. Cache I/O is
// best-effort — failures are swallowed so a transient FS hiccup
// doesn't surface to the user as a toolbar error.
//
// Read-on-open hydration is **not** owned by this hook — it's a
// separate file-lifecycle concern that runs once per tab mount
// (carries to the editor pane wiring slice).

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
      // Inner persist runs first so a roll-back-on-failure leaves the
      // cache untouched. The throw bubbles up to the caller and skips
      // the cache I/O below.
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
        // Best-effort cache layer — the toolbar already shows the
        // committed `auto_capture` flag, so a cache write/delete
        // failure shouldn't make the UI lie about the toggle.
      }
    },
    [setInner, vaultPath, filePath],
  );
  return { ...inner, setAutoCapture };
}
