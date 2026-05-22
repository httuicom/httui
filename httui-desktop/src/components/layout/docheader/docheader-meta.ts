export interface AuthorInfo {
  /** First commit author of the file. `null` when the file isn't
   *  tracked or git lookup failed. */
  name: string | null;
  email: string | null;
}

export function authorInitialsFromFirstCommit(info: AuthorInfo): string {
  const name = info.name?.trim() ?? "";
  if (name.length === 0) {
    // Fall back to the first letter of the email's local part.
    const email = info.email?.trim() ?? "";
    if (email.length === 0) return "?";
    const local = email.split("@")[0] ?? "";
    return local.slice(0, 2).toUpperCase() || "?";
  }
  const parts = name.split(/\s+/u).filter((p) => p.length > 0);
  if (parts.length === 0) return "?";
  if (parts.length === 1) return parts[0]!.slice(0, 2).toUpperCase();
  return (parts[0]![0]! + parts[parts.length - 1]![0]!).toUpperCase();
}

export function formatEditedTime(
  mtimeMs: number | null,
  dirty: boolean,
  now: number = Date.now(),
): string {
  if (mtimeMs === null) {
    return dirty ? "Edited just now" : "Not yet saved";
  }
  const diffSec = Math.max(0, Math.floor((now - mtimeMs) / 1000));
  const suffix = dirty ? " · unsaved" : "";
  if (diffSec < 60) return `Edited just now${suffix}`;
  if (diffSec < 3600) return `Edited ${Math.floor(diffSec / 60)}m ago${suffix}`;
  if (diffSec < 86400)
    return `Edited ${Math.floor(diffSec / 3600)}h ago${suffix}`;
  return `Edited ${Math.floor(diffSec / 86400)}d ago${suffix}`;
}

export interface BranchSummaryData {
  branch: string | null;
  /** Lines added in the file since branch base. */
  addedLines: number;
  /** Lines modified in the file since branch base. */
  modifiedLines: number;
}

export function formatBranchSummary(data: BranchSummaryData): string {
  const branch = data.branch ?? "(detached)";
  const parts: string[] = [`Branch ${branch}`];
  if (data.addedLines > 0) parts.push(`+${data.addedLines}`);
  if (data.modifiedLines > 0) parts.push(`~${data.modifiedLines}`);
  return parts.join(" ");
}

export interface LastRunSummary {
  /** ISO-8601 timestamp of the most recent run. `null` when the file
   *  has no recorded runs in `block_run_history`. */
  ranAt: string | null;
  blockCount: number;
  failedCount: number;
}

export function formatLastRun(summary: LastRunSummary): string {
  if (summary.ranAt === null || summary.blockCount === 0) {
    return "No runs yet";
  }
  const time = formatHHMM(summary.ranAt);
  const blocks = `${summary.blockCount} block${summary.blockCount === 1 ? "" : "s"}`;
  const failed =
    summary.failedCount > 0 ? ` · ${summary.failedCount} failed` : "";
  return `Last run ${time} · ${blocks}${failed}`;
}

export type LastRunTone = "ok" | "fail" | "muted";

export function lastRunTone(summary: LastRunSummary): LastRunTone {
  if (summary.ranAt === null || summary.blockCount === 0) return "muted";
  return summary.failedCount > 0 ? "fail" : "ok";
}

function formatHHMM(iso: string): string {
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return iso;
  const d = new Date(t);
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  return `${hh}:${mm}`;
}
