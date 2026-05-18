// Tauri wrapper for `scan_vault_tags_cmd`.
//
// Powers `useTagIndexStore`'s vault-open and post-save refreshes.
// Pure `invoke()` shell; coverage from the inline test below + the
// store consumer tests that mock the command name.

import { invoke } from "@tauri-apps/api/core";

export interface TagEntry {
  /** Path relative to the vault root, with forward slashes. */
  path: string;
  tags: string[];
}

export function scanVaultTags(vaultPath: string): Promise<TagEntry[]> {
  return invoke("scan_vault_tags_cmd", { vaultPath });
}
