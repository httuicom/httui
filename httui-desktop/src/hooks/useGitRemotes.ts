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
