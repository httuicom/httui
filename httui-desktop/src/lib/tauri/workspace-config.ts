// coverage:exclude file — Pure `invoke()` wrapper + IPC type
// declarations. Same rationale as commands.ts (audit-002): testing
// these is testing the mock harness; behavior lives in the backend.
//
// V3 source-tracking surface for workspace config. Lives in its own
// module so commands.ts stays under the SOLID size gate.
//
// Mirrors `httui_core::vault_config::workspace::{WorkspaceSources,
// WorkspaceDefaultsWithSources}`.

import { invoke } from "@tauri-apps/api/core";

import type { WorkspaceDefaults } from "./commands";

/** Per-field origin marker. `local` means the value lives in
 * `workspace.local.toml` — surfaces the "overridden locally" badge
 * in the Settings UI (V3 cenário 3). */
export type WorkspaceFieldSource = "workspace" | "local";

export interface WorkspaceSources {
  environment: WorkspaceFieldSource;
  git_remote: WorkspaceFieldSource;
  git_branch: WorkspaceFieldSource;
  display_name: WorkspaceFieldSource;
}

export interface WorkspaceDefaultsWithSources {
  defaults: WorkspaceDefaults;
  sources: WorkspaceSources;
}

/** Same as `getWorkspaceConfig` but also returns per-field origin
 * (`workspace` vs `local`). Used by the Settings UI to render the
 * "overridden locally" badges (V3 cenário 3). */
export function getWorkspaceConfigWithSources(
  vaultPath: string,
): Promise<WorkspaceDefaultsWithSources> {
  return invoke("get_workspace_config_with_sources", { vaultPath });
}
