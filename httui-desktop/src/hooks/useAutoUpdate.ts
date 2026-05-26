import { useEffect } from "react";
import { check } from "@tauri-apps/plugin-updater";
import { ask } from "@tauri-apps/plugin-dialog";
import { useSettingsStore } from "@/stores/settings";
import { shouldOfferUpdate } from "@/lib/updater/should-offer-update";

export function useAutoUpdate() {
  useEffect(() => {
    async function checkForUpdate() {
      try {
        const update = await check();
        if (!update) return;

        const includePrereleases =
          useSettingsStore.getState().autoUpdateIncludePrereleases;
        if (!shouldOfferUpdate(update.version, includePrereleases)) return;

        const yes = await ask(
          `A new version ${update.version} is available. Would you like to update now?`,
          { title: "Update Available", kind: "info" },
        );

        if (yes) {
          await update.downloadAndInstall();
          const { relaunch } = await import("@tauri-apps/plugin-process");
          await relaunch();
        }
      } catch {
        // Silent fail — don't bother user if update check fails
      }
    }

    // Check after 3 seconds to not block startup
    const timer = setTimeout(checkForUpdate, 3000);
    return () => clearTimeout(timer);
  }, []);
}
