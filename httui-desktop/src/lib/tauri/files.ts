// Per-vault file metadata wrappers: mtime + per-file workspace settings.
// Coverage via consumer hooks (`useFileMtime`, `useFileAutoCapture`) that mock the Tauri command names.

import { invoke } from "@tauri-apps/api/core";

/** Last modification timestamp for a vault note in epoch milliseconds.
 * `null` if the file is absent or its mtime can't be read. */
export function getFileMtime(
  vaultPath: string,
  filePath: string,
): Promise<number | null> {
  return invoke("get_file_mtime", { vaultPath, filePath });
}

/** Per-file workspace settings persisted under
 * `[files."<file_path>"]` in `.httui/workspace.toml`.
 * `docheader_compact` is optional on the wire (Rust skips default booleans);
 * treat `undefined` as `false` at every read site. */
export interface FileSettings {
  auto_capture: boolean;
  docheader_compact?: boolean;
}

/** Read the per-file workspace settings entry. Returns the
 * `Default::default()` value (auto_capture = false) when no entry
 * exists for `file_path`. */
export function getFileSettings(
  vaultPath: string,
  filePath: string,
): Promise<FileSettings> {
  return invoke("get_file_settings", { vaultPath, filePath });
}

/** Set the auto-capture flag for `file_path`. Writes through to
 * `workspace.toml` (base, never `.local.toml`). Default-valued
 * entries are pruned so the file stays minimal. */
export function setFileAutoCapture(
  vaultPath: string,
  filePath: string,
  autoCapture: boolean,
): Promise<void> {
  return invoke("set_file_auto_capture", {
    vaultPath,
    filePath,
    autoCapture,
  });
}

/** Set the DocHeader compact-mode flag for `file_path`. Same prune
 * semantics as `setFileAutoCapture`. Powers. */
export function setFileDocheaderCompact(
  vaultPath: string,
  filePath: string,
  compact: boolean,
): Promise<void> {
  return invoke("set_file_docheader_compact", {
    vaultPath,
    filePath,
    compact,
  });
}
