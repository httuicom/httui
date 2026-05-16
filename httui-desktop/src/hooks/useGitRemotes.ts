// V10.1 — store-backed shim. Remotes are now polled by
// `useGitStore` on the same 2s cadence as status (V10 had a manual
// re-poll interval in GitPanelContainer; the store absorbs it so a
// `git remote add` done outside the app still reflects). Public
// shape unchanged for `useShareRepoUrl` + GitPanelContainer.

import { useEffect } from "react";

import { useGitStore } from "@/stores/git";
import type { Remote } from "@/lib/tauri/git";

export interface UseGitRemotesResult {
  remotes: Remote[];
  loaded: boolean;
  error: string | null;
  refresh: () => void;
}

export function useGitRemotes(vaultPath: string | null): UseGitRemotesResult {
  useEffect(() => {
    useGitStore.getState().acquire(vaultPath);
    return () => useGitStore.getState().release();
  }, [vaultPath]);

  const remotes = useGitStore((s) => s.remotes);
  const loaded = useGitStore((s) => s.remotesLoaded);
  const error = useGitStore((s) => s.remotesError);
  const refresh = useGitStore((s) => s.refreshRemotes);

  return { remotes, loaded, error, refresh };
}
