import { useEffect } from "react";

import { useConfigChangeRefresh } from "@/hooks/useConfigChangeRefresh";

/**
 * Keep a config-file-backed store resource synced for the lifetime of
 * the consumer: refresh once on mount, then again whenever the backend
 * emits `config-changed` for `category` (an external `*.toml` edit
 * picked up by the file watcher).
 *
 * This is the exact mount-refresh + config-watch *pair* that the
 * Connections / Environments / Variables PageContainers each
 * re-implemented verbatim (audit 05 §A.2 §1: "initial-load effect" +
 * "config-changed listener effect"). The two always travel together
 * over the same `refresh` fn, so they are one cohesive unit.
 *
 * Composes the tested `useConfigChangeRefresh` primitive rather than
 * replacing it — that hook stays the building block (its async-listen
 * race handling and its own tests are unchanged).
 *
 * `refresh` must be a stable reference (a store action selected via
 * `useXStore((s) => s.refresh)` is — Zustand actions are stable), or
 * the mount effect re-fires on every render.
 */
export function useConfigSyncedResource(
  category: string,
  refresh: () => void | Promise<void>,
): void {
  useEffect(() => {
    void refresh();
  }, [refresh]);

  useConfigChangeRefresh(category, refresh);
}
