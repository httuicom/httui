// Tauri wrapper for `grep_var_uses`.
//
// Pure `invoke()` shell. Coverage comes from the
// `<UsedInBlocksList>` consumer tests (which mock the command name).

import { invoke } from "@tauri-apps/api/core";

export interface VarUseEntry {
  /** Path relative to the vault root, with forward slashes. */
  file_path: string;
  /** 1-indexed line number where the match occurred. */
  line: number;
  /** Trimmed line content; truncated to ~120 chars with `…`. */
  snippet: string;
}

export function grepVarUses(
  vaultPath: string,
  key: string,
): Promise<VarUseEntry[]> {
  return invoke("grep_var_uses", {
    vaultPath,
    key,
  });
}
