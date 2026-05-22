// Autocomplete suggestion provider for the pre-flight check popover.
// `connection` → vault connection names; `env_var` → active env keys; others → empty.

import { listConnections } from "@/lib/tauri/connections";
import { useEnvironmentStore } from "@/stores/environment";

import type { PreflightCheckKind } from "@/lib/blocks/preflight-checks";

export type SuggestionProvider = (
  kind: PreflightCheckKind,
) => Promise<string[]>;

/** Default provider. Tests inject their own to avoid live stores / Tauri. */
export const defaultSuggestionProvider: SuggestionProvider = async (kind) => {
  switch (kind) {
    case "connection":
      try {
        const list = await listConnections();
        return list.map((c) => c.name).filter((n) => n.length > 0);
      } catch {
        return [];
      }
    case "env_var":
      try {
        const vars = await useEnvironmentStore.getState().getActiveVariables();
        return Object.keys(vars).sort();
      } catch {
        return [];
      }
    case "branch":
    case "file_exists":
    case "command":
      return [];
  }
};
