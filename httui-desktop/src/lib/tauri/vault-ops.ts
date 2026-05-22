// coverage:exclude file — pure invoke() wrappers + IPC types.

import { invoke } from "@tauri-apps/api/core";

import type { ScaffoldReport } from "./commands";

export interface CloneOutcome {
  /** Absolute path of the cloned repo, ready for switchVault. */
  destination: string;
}

/**
 * `git clone <url> <parent>/<repo-name>`. Auth (HTTPS PAT, SSH keys)
 * is delegated to the user's git credential helper / ssh-agent.
 *
 * `parent` is the *container* folder the user picked. When it is
 * `null` the backend defaults to `~/Documents`. The repo's leaf
 * name is always derived from the URL — picking `/tmp` clones into
 * `/tmp/<repo>` rather than overwriting `/tmp` itself.
 */
export function cloneVault(
  url: string,
  parent: string | null,
): Promise<CloneOutcome> {
  return invoke("clone_vault_cmd", { url, parent });
}

export interface CreateOutcome {
  /** Absolute path of the new vault, ready for switchVault. */
  destination: string;
  /** What the scaffold actually wrote. */
  scaffold: ScaffoldReport;
}

/**
 * Create a brand-new vault at `<parentPath>/<name>` — composes
 * mkdir + `git init` + scaffold. Backend rejects empty/path-traversal
 * names and refuses to overwrite an existing non-empty folder.
 */
export function createVault(
  parentPath: string,
  name: string,
): Promise<CreateOutcome> {
  return invoke("create_vault_cmd", { parentPath, name });
}

/**
 * Persist a secret in the OS keychain.
 * Called once per `MissingRef` the user fills in inside the
 * first-run secrets modal. Empty key/value pairs are rejected at
 * the backend.
 */
export function saveSecret(keychainKey: string, value: string): Promise<void> {
  return invoke("save_secret_cmd", { keychainKey, value });
}
