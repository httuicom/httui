import { invoke } from "@tauri-apps/api/core";
import { useSettingsStore } from "@/stores/settings";

/** One aggregated daily count for a tracked feature. Mirrors the Rust
 * `FeatureUsage` struct (`httui_core::db::feature_usage`). */
export interface FeatureUsage {
  date: string;
  feature: string;
  count: number;
}

/** Features the local usage dashboard records. Kept in sync with the
 * backend allowlist in `commands/telemetry.rs`. */
export type TrackedFeature = "http_block_run" | "db_block_run";

/**
 * Record one usage tick for `feature` — but only when the user has opted
 * in (`telemetryEnabled`). Fire-and-forget: never throws into the caller's
 * execution path, so a telemetry failure can't break a block run.
 */
export function recordFeatureUsage(feature: TrackedFeature): void {
  if (!useSettingsStore.getState().telemetryEnabled) return;
  void invoke("record_feature_usage", { feature }).catch(() => {});
}

/** Daily per-feature counts in the inclusive `[from, to]` range (ISO dates). */
export function getFeatureUsage(
  from: string,
  to: string,
): Promise<FeatureUsage[]> {
  return invoke<FeatureUsage[]>("get_feature_usage", { from, to });
}

/** Wipe all locally recorded usage. Backs the dashboard reset control. */
export function clearFeatureUsage(): Promise<void> {
  return invoke<void>("clear_feature_usage");
}
