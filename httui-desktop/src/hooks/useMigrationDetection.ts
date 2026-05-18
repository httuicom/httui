// Decides whether the empty-state should show the MVP-to-v1
// migration banner. Combines the filesystem probe
// (`detect_vault_migration` Tauri command) with the user-pref
// dismissal flag from `useSettingsStore`.
//
// Carry-over Slice 1 shipped the dismissal
// pref schema; this hook is slice 2's consumer surface. Slice 3
// (AppShell mount) reads `shouldShowBanner` and renders
// `<MigrationBanner>` accordingly.

import { useCallback, useEffect, useRef, useState } from "react";

import {
  detectVaultMigration,
  shouldPromptMigration,
  type MigrationCandidate,
} from "@/lib/tauri/migration";
import { useSettingsStore } from "@/stores/settings";

export interface UseMigrationDetectionResult {
  /** Backend probe result (or `null` while still loading / disabled). */
  candidate: MigrationCandidate | null;
  /** True iff the banner should be visible: legacy db present, v1
   * layout absent, AND user hasn't dismissed it. */
  shouldShowBanner: boolean;
  /** Persistently dismiss the banner. Goes through
   * `useSettingsStore.setMvpMigrationDismissed(true)`. */
  dismiss: () => void;
  /** Re-run the backend probe. Useful after the user runs the
   * migration successfully (slice 4) so the banner clears. */
  refresh: () => void;
}

/** Probe a vault path and combine with the dismissal pref. Pass
 * `null` (no vault picked yet) to disable the probe — the hook
 * returns `{ candidate: null, shouldShowBanner: false }` and the
 * effect is idle. */
export function useMigrationDetection(
  vaultPath: string | null,
): UseMigrationDetectionResult {
  const [candidate, setCandidate] = useState<MigrationCandidate | null>(null);
  const cancelledRef = useRef(false);

  const dismissed = useSettingsStore((s) => s.mvpMigrationDismissed);
  const setMvpMigrationDismissed = useSettingsStore(
    (s) => s.setMvpMigrationDismissed,
  );

  const fetchOnce = useCallback(async () => {
    if (!vaultPath) {
      setCandidate(null);
      return;
    }
    try {
      const next = await detectVaultMigration(vaultPath);
      if (cancelledRef.current) return;
      setCandidate(next);
    } catch {
      if (cancelledRef.current) return;
      // A failed probe is "no banner" — don't surface a phantom
      // migration prompt because of an FS hiccup.
      setCandidate(null);
    }
  }, [vaultPath]);

  useEffect(() => {
    cancelledRef.current = false;
    void fetchOnce();
    return () => {
      cancelledRef.current = true;
    };
  }, [fetchOnce]);

  const refresh = useCallback(() => {
    void fetchOnce();
  }, [fetchOnce]);

  const dismiss = useCallback(() => {
    setMvpMigrationDismissed(true);
  }, [setMvpMigrationDismissed]);

  const shouldShowBanner =
    candidate !== null && shouldPromptMigration(candidate) && !dismissed;

  return { candidate, shouldShowBanner, dismiss, refresh };
}
