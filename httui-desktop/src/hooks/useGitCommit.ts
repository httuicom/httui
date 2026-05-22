import { useCallback, useState } from "react";

import { gitCommit } from "@/lib/tauri/git";
import { useGitStore } from "@/stores/git";

export interface UseGitCommitResult {
  committing: boolean;
  commit: (input: { message: string; amend: boolean }) => Promise<void>;
}

export function useGitCommit(vaultPath: string | null): UseGitCommitResult {
  const [committing, setCommitting] = useState(false);
  const refreshStatus = useGitStore((s) => s.refreshStatus);
  const resetCommitMessage = useGitStore((s) => s.resetCommitMessage);

  const commit = useCallback(
    async (input: { message: string; amend: boolean }) => {
      if (!vaultPath || committing) return;
      setCommitting(true);
      try {
        await gitCommit(vaultPath, input.message, input.amend);
        resetCommitMessage();
        await refreshStatus();
      } finally {
        setCommitting(false);
      }
    },
    [vaultPath, committing, refreshStatus, resetCommitMessage],
  );

  return { committing, commit };
}
