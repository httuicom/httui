// Reads + writes the per-file `docheader_compact` flag persisted in
// `.httui/workspace.toml` via `get_file_settings` /
// `set_file_docheader_compact` Tauri commands. Powers Epic 50 Story
// 06 — `<DocHeaderCard>` click-on-title flips compact mode and the
// preference survives vault reopen.
//
// Mirrors `useFileAutoCapture` (commit 78f7a81 / earlier): idle when
// either path is null, optimistic update on toggle, rollback to
// on-disk truth on persist failure. The store mutates the **base**
// workspace.toml (audit-003) so `.local.toml` overrides survive.

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
      // `docheader_compact` is optional on the wire (Rust skips the
      // field when it's at the default). Treat undefined as false.
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
        // Roll back to on-disk truth so the UI doesn't drift.
        setLocal(prev);
        throw e;
      }
    },
    [vaultPath, filePath, compact],
  );

  return { compact, loaded, setCompact };
}
