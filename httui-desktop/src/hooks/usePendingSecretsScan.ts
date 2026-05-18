// first-run secrets scan trigger.
//
// Watches `vaultPath` and, on every transition into a defined vault,
// calls `list_missing_secrets` and pushes the result into the
// `pendingSecrets` store. The modal subscribes to the store and
// opens automatically when the list is non-empty.
//
// Failures are logged and swallowed — a vault with no readable
// `connections.toml` (e.g. the freshly-cloned Hello-World) shouldn't
// block the workbench from opening. The MissingRef scanner already
// returns `Ok([])` for vaults without refs, so the only failure
// path is something exceptional (permission, IO).

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
