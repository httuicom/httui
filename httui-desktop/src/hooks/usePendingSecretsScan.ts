import { useEffect } from "react";

import { listMissingSecrets } from "@/lib/tauri/commands";
import { usePendingSecretsStore } from "@/stores/pendingSecrets";
import { useWorkspaceStore } from "@/stores/workspace";

export function usePendingSecretsScan(): void {
  const vaultPath = useWorkspaceStore((s) => s.vaultPath);
  const setPending = usePendingSecretsStore((s) => s.setPending);
  const reset = usePendingSecretsStore((s) => s.reset);

  useEffect(() => {
    if (!vaultPath) {
      reset();
      return;
    }
    let cancelled = false;
    listMissingSecrets(vaultPath)
      .then((refs) => {
        if (cancelled) return;
        setPending(refs);
      })
      .catch((err) => {
        // Don't surface — secrets prompt is a nicety, not a blocker.
        console.error("listMissingSecrets failed:", err);
      });
    return () => {
      cancelled = true;
    };
  }, [vaultPath, setPending, reset]);
}
