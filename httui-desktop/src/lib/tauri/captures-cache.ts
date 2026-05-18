// Tauri wrappers for the captures cache.
//
// When auto-capture is ON for a file, the consumer (Capture store
// + footer UI) filters secret-flagged entries out, JSON-stringifies
// the captures map, and writes it via `writeCapturesCache`. On app
// start each open file calls `readCapturesCache` to seed the
// in-memory store. Toggling auto-capture OFF deletes the file via
// `deleteCapturesCache`.
//
// JSON shape is owned by the consumer — backend treats it as opaque.

import { invoke } from "@tauri-apps/api/core";

export function readCapturesCache(
  vaultPath: string,
  filePath: string,
): Promise<string | null> {
  return invoke("read_captures_cache_cmd", { vaultPath, filePath });
}

export function writeCapturesCache(
  vaultPath: string,
  filePath: string,
  json: string,
): Promise<string> {
  return invoke("write_captures_cache_cmd", { vaultPath, filePath, json });
}

export function deleteCapturesCache(
  vaultPath: string,
  filePath: string,
): Promise<boolean> {
  return invoke("delete_captures_cache_cmd", { vaultPath, filePath });
}
