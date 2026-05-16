// V10.1 — store-backed shim. The polling + state moved into
// `useGitStore` so the pane-tab and the GitSidePanel share one
// source (cenário 7). The public shape is unchanged so V10
// consumers (StatusBar, DocHeaderedEditor, GitPanelContainer)
// keep working untouched.

import { useEffect } from "react";

import { useGitStore } from "@/stores/git";
import type { GitStatus } from "@/lib/tauri/git";

export { GIT_STATUS_POLL_MS } from "@/stores/git";

export interface UseGitStatusResult {
  status: GitStatus | null;
  error: string | null;
  refresh: () => void;
}

export function useGitStatus(vaultPath: string | null): UseGitStatusResult {
  useEffect(() => {
    useGitStore.getState().acquire(vaultPath);
    return () => useGitStore.getState().release();
  }, [vaultPath]);

  const status = useGitStore((s) => s.status);
  const error = useGitStore((s) => s.statusError);
  const refresh = useGitStore((s) => s.refreshStatus);

  return { status, error, refresh };
}
