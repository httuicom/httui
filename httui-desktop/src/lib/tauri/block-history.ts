// coverage:exclude file — pure invoke() wrappers + IPC types.
//
// Tauri wrappers for `block_run_history` (Story 24.6 + Epic 50
// Story 03 last-run summary). Extracted from `commands.ts` when
// the parent file crossed the 600-line size gate (Epic 30a Story
// 07 split anticipated this — see tech-debt.md).

import { invoke } from "@tauri-apps/api/core";

export interface HistoryEntry {
  id: number;
  file_path: string;
  block_alias: string;
  method: string;
  url_canonical: string;
  status: number | null;
  request_size: number | null;
  response_size: number | null;
  elapsed_ms: number | null;
  outcome: string;
  ran_at: string;
  /** EXPLAIN ANALYZE plan captured for this run (Epic 53 Story 01).
   * Stored as the JSON-stringified plan value (Postgres JSON or
   * MySQL EXPLAIN-as-text after backend cap). Optional because:
   * - Rust serde skips the field with
   *   `skip_serializing_if = "Option::is_none"`, so non-EXPLAIN
   *   runs don't emit it on the wire;
   * - blocks without `explain=true` info-string never populate it. */
  plan?: string;
}

export interface InsertHistoryEntry {
  file_path: string;
  block_alias: string;
  method: string;
  url_canonical: string;
  status: number | null;
  request_size: number | null;
  response_size: number | null;
  elapsed_ms: number | null;
  outcome: string;
  /** Optional EXPLAIN plan blob (Epic 53 Story 01). Pass when the
   * block ran with `explain=true` info-string and the executor
   * returned a non-null `DbResponse.plan` — JSON.stringify the
   * value before passing here. Omit / pass undefined for regular
   * runs so the column stays NULL. */
  plan?: string;
}

export function listBlockHistory(
  filePath: string,
  blockAlias: string,
): Promise<HistoryEntry[]> {
  return invoke("list_block_history", { filePath, blockAlias });
}

/** Return the most recent N runs for `filePath` across every alias.
 *  Powers the Epic 29 sidebar History tab. Pass `limit <= 0` to fall
 *  back to the 50-entry default. */
export function listBlockHistoryForFile(
  filePath: string,
  limit: number,
): Promise<HistoryEntry[]> {
  return invoke("list_block_history_for_file", { filePath, limit });
}

/** Backend mirror of `LastRunSummary` from
 *  `components/layout/docheader/docheader-meta.ts`. The Rust
 *  serde shape uses snake_case. */
export interface LastRunSummaryRaw {
  ran_at: string | null;
  block_count: number;
  failed_count: number;
}

/** Aggregate `block_run_history` rows for a file into the "last
 *  run-all session" summary. Powers Epic 50 Story 03's
 *  `<DocHeaderMetaStrip>` Last-run chip. The 5s session window
 *  heuristic happens server-side; the consumer just maps
 *  `ran_at → ranAt` and renders. */
export function blockHistoryLastRunSummary(
  filePath: string,
): Promise<LastRunSummaryRaw> {
  return invoke("block_history_last_run_summary", { filePath });
}

export function insertBlockHistory(entry: InsertHistoryEntry): Promise<void> {
  return invoke("insert_block_history", { entry });
}

export function purgeBlockHistory(
  filePath: string,
  blockAlias: string,
): Promise<number> {
  return invoke("purge_block_history", { filePath, blockAlias });
}
