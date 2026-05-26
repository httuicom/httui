// branch list + checkout actions for the status-bar
// BranchMenu. Lazy: `loadBranches()` is called when the menu opens
// (branches change rarely, polling is overkill — matches the
// `useGitRemotes` posture). `selectBranch` / `createBranch` route to
// the existing git_checkout / git_checkout_b commands, then refresh
// the workspace file tree so the sidebar reflects the new branch.
//
// Idle when `vaultPath` is null. `busy` guards against double-fire
// while a checkout is in flight (git locks the index anyway, but the
// UI shouldn't queue a second checkout).

import { useCallback, useRef, useState } from "react";

import {
  gitBranchList,
  gitCheckout,
  gitCheckoutB,
  type BranchInfo,
} from "@/lib/tauri/git";
import { useWorkspaceStore } from "@/stores/workspace";

export interface UseGitBranchActionsResult {
  branches: BranchInfo[];
  busy: boolean;
  error: string | null;
  loadBranches: () => void;
  selectBranch: (branch: BranchInfo) => Promise<void>;
  createBranch: (name: string) => Promise<void>;
}

export function useGitBranchActions(
  vaultPath: string | null,
): UseGitBranchActionsResult {
  const [branches, setBranches] = useState<BranchInfo[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const cancelledRef = useRef(false);

  const loadBranches = useCallback(() => {
    if (!vaultPath) {
      setBranches([]);
      return;
    }
    cancelledRef.current = false;
    void (async () => {
      try {
        const list = await gitBranchList(vaultPath);
        if (cancelledRef.current) return;
        setBranches(list);
        setError(null);
      } catch (e) {
        if (cancelledRef.current) return;
        setBranches([]);
        setError(e instanceof Error ? e.message : String(e));
      }
    })();
  }, [vaultPath]);

  const runCheckout = useCallback(
    async (op: () => Promise<void>) => {
      if (!vaultPath || busy) return;
      setBusy(true);
      setError(null);
      try {
        await op();
        await useWorkspaceStore.getState().refreshFileTree(vaultPath);
        const list = await gitBranchList(vaultPath);
        setBranches(list);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        setBusy(false);
      }
    },
    [vaultPath, busy],
  );

  const selectBranch = useCallback(
    (branch: BranchInfo) =>
      runCheckout(() => gitCheckout(vaultPath!, branch.name)),
    [runCheckout, vaultPath],
  );

  const createBranch = useCallback(
    (name: string) => runCheckout(() => gitCheckoutB(vaultPath!, name)),
    [runCheckout, vaultPath],
  );

  return { branches, busy, error, loadBranches, selectBranch, createBranch };
}
