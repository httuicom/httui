import { useCallback } from "react";

import { stagePath, unstagePath, type GitFileChange } from "@/lib/tauri/git";
import { useGitStore } from "@/stores/git";

export interface UseGitStageResult {
  toggleStage: (file: GitFileChange) => Promise<void>;
}

export function useGitStage(vaultPath: string | null): UseGitStageResult {
  const refreshStatus = useGitStore((s) => s.refreshStatus);

  const toggleStage = useCallback(
    async (file: GitFileChange) => {
      if (!vaultPath) return;
      try {
        if (file.staged) {
          await unstagePath(vaultPath, file.path);
        } else {
          await stagePath(vaultPath, file.path);
        }
      } finally {
        await refreshStatus();
      }
    },
    [vaultPath, refreshStatus],
  );

  return { toggleStage };
}
