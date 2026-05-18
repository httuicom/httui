// Tauri wrappers for the run-body filesystem cache.
//
// Powers the streamed-execution path's post-run persistence and the
// run-diff viewer's history reads. Bytes flow through `Vec<u8>` /
// `Uint8Array` so binary HTTP responses survive the IPC boundary
// untouched.

import { invoke } from "@tauri-apps/api/core";

export type RunBodyKind = "json" | "bin";

export interface RunBodyEntry {
  run_id: string;
  /** "json" | "bin" — chosen by the writer at insert time. */
  kind: string;
  byte_size: number;
  /** True when the writer hit the 1 MiB cap and appended the
   * truncation marker. */
  truncated: boolean;
}

export function writeRunBody(
  vaultPath: string,
  filePath: string,
  alias: string,
  runId: string,
  kind: RunBodyKind,
  body: Uint8Array | number[],
): Promise<string> {
  return invoke("write_run_body_cmd", {
    vaultPath,
    filePath,
    alias,
    runId,
    kind,
    body: Array.from(body),
  });
}

export async function readRunBody(
  vaultPath: string,
  filePath: string,
  alias: string,
  runId: string,
): Promise<Uint8Array | null> {
  const result = await invoke<number[] | null>("read_run_body_cmd", {
    vaultPath,
    filePath,
    alias,
    runId,
  });
  return result ? new Uint8Array(result) : null;
}

export function listRunBodies(
  vaultPath: string,
  filePath: string,
  alias: string,
): Promise<RunBodyEntry[]> {
  return invoke("list_run_bodies_cmd", { vaultPath, filePath, alias });
}

export function trimRunBodies(
  vaultPath: string,
  filePath: string,
  alias: string,
  keepN: number,
): Promise<number> {
  return invoke("trim_run_bodies_cmd", {
    vaultPath,
    filePath,
    alias,
    keepN,
  });
}

/** Move every cached run body for `(filePath, oldAlias)` to
 *  `(filePath, newAlias)`. Resolves to `false` when the source dir
 *  is missing (no runs to move); rejects when the destination dir
 * already has cached runs. Powers the alias-rename
 *  flow that fires when the user edits a block's `alias=` info-
 *  string token. */
export function renameAliasRuns(
  vaultPath: string,
  filePath: string,
  oldAlias: string,
  newAlias: string,
): Promise<boolean> {
  return invoke("rename_alias_runs_cmd", {
    vaultPath,
    filePath,
    oldAlias,
    newAlias,
  });
}
