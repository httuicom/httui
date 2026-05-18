// TS bridge to `preflight_commands::evaluate_preflight_cmd`.
//
// Mirrors the Rust `EvaluatedPreflightItem` shape verbatim so the
// React layer can render `PreflightPills` directly off the response.
// The pure layer (`@/components/blocks/preflight/preflight-types`)
// already declares the matching `CheckResult` discriminated union.

import { invoke } from "@tauri-apps/api/core";

import type { CheckResult } from "@/components/blocks/preflight/preflight-types";

export type PreflightItemKind =
  | "connection"
  | "env_var"
  | "branch"
  | "file_exists"
  | "command"
  | "unknown";

export interface EvaluatedPreflightItem {
  kind: PreflightItemKind;
  /** Short label used inside the pill — the connection name, the
   *  env-var name, the file path, the first command token, etc. */
  label: string;
  result: CheckResult;
}

/** Read `file_path`'s frontmatter, parse the `preflight:` block, and
 *  evaluate every item against host state (FS + process for
 *  `file_exists` / `command`; empty-context fail for the others
 *  until the follow-up wires connections / env / keychain). */
export function evaluatePreflight(
  filePath: string,
  vaultPath: string,
): Promise<EvaluatedPreflightItem[]> {
  return invoke<EvaluatedPreflightItem[]>("evaluate_preflight_cmd", {
    filePath,
    vaultPath,
  });
}
