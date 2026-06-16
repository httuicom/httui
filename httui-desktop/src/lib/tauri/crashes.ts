import { invoke } from "@tauri-apps/api/core";

/** One crash log's metadata. Mirrors the Rust `CrashLog` struct
 * (`httui_core::crash_log`). The body is fetched separately. */
export interface CrashLog {
  /** File name — the id passed to `readCrashLog`. */
  name: string;
  /** Origin tag (e.g. `desktop`, `lsp`). */
  source: string;
  /** Capture time, Unix epoch milliseconds. */
  epoch_ms: number;
  /** First non-empty line of the body, for the list preview. */
  summary: string;
}

/** List local crash logs, newest-first. */
export function listCrashLogs(): Promise<CrashLog[]> {
  return invoke<CrashLog[]>("list_crash_logs");
}

/** Read one crash log's full body by file name. */
export function readCrashLog(name: string): Promise<string> {
  return invoke<string>("read_crash_log", { name });
}

/** Delete every local crash log. */
export function clearCrashLogs(): Promise<void> {
  return invoke<void>("clear_crash_logs");
}
