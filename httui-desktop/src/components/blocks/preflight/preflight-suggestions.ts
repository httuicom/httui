// V6 / cenário 9 — suggestion provider for the pre-flight check
// builder popover. Returns autocomplete candidates per kind.
//
// MVP scope:
//   - `connection` → vault connection names (Tauri `listConnections`).
//   - `env_var`    → active environment's variable keys (store).
//   - others       → empty list (text-only input).
//
// Pure data-fetcher. The popover renders the dropdown below the value
// input and filters by substring match on the typed text.

import { listConnections } from "@/lib/tauri/connections";
import { useEnvironmentStore } from "@/stores/environment";

import type { PreflightCheckKind } from "@/lib/blocks/preflight-checks";

export type SuggestionProvider = (
  kind: PreflightCheckKind,
) => Promise<string[]>;

/** Default provider used by the inline DocHeader builder. Tests pass
 *  their own provider to inject deterministic data without touching
 *  the live stores / Tauri layer. */
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
    case "keychain":
    case "file_exists":
    case "command":
    case "unknown" as PreflightCheckKind:
      return [];
  }
};
