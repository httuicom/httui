// Pure derivation helpers: connections + enrichment → list rows, kind counts,
// env summaries, and status counts. No React, no Tauri.

import type { Connection } from "@/lib/tauri/connections";

import type { ListRowItem } from "./ConnectionListRow";
import type { EnvSummary } from "./ConnectionsKindSidebar";
import type { ListStatusCounts } from "./ConnectionsListPanel";
import {
  CONNECTION_KIND_ORDER,
  kindFromDriver,
  type ConnectionKind,
} from "./connection-kinds";

export interface ConnectionEnrichment {
  /** Connection id from the backend. */
  id: string;
  env: string | null;
  latencyMs: number | null;
  uses: number;
}

/** Latency above this threshold renders the dot yellow. */
export const SLOW_LATENCY_MS = 200;

/** True when the name contains a word-bounded `prod` (case-insensitive). */
export function isProductionName(name: string): boolean {
  return /\bprod(uction)?\b/i.test(name);
}

/** Map the row-level latency to a canvas-spec status intent. */
export function statusFromLatency(
  latencyMs: number | null,
): ListRowItem["status"] {
  if (latencyMs === null) return "untested";
  if (latencyMs < 0) return "down";
  if (latencyMs >= SLOW_LATENCY_MS) return "slow";
  return "ok";
}

interface BuildArgs {
  connections: Connection[];
  enrichment?: ConnectionEnrichment[];
  /** Optional kind filter from the sidebar. `null` = "all". */
  kindFilter?: ConnectionKind | null;
  /** Case-insensitive substring filter across name, host, database_name. */
  search?: string;
}

/** Build list-row items from raw connections + enrichment. */
export function buildListRows({
  connections,
  enrichment = [],
  kindFilter = null,
  search = "",
}: BuildArgs): ListRowItem[] {
  const enrichById = new Map(enrichment.map((e) => [e.id, e] as const));
  const q = search.trim().toLowerCase();
  return connections
    .map((c) => {
      const e = enrichById.get(c.id);
      const kind = kindFromDriver(c.driver);
      return {
        id: c.id,
        name: c.name,
        kind,
        host: c.host,
        env: e?.env ?? null,
        latencyMs: e?.latencyMs ?? null,
        status: statusFromLatency(e?.latencyMs ?? null),
        uses: e?.uses ?? 0,
        isProd: isProductionName(c.name),
      } satisfies ListRowItem;
    })
    .filter((r) => (kindFilter === null ? true : r.kind === kindFilter))
    .filter((r) => {
      if (q.length === 0) return true;
      return (
        r.name.toLowerCase().includes(q) ||
        (r.host !== null && r.host.toLowerCase().includes(q)) ||
        (r.env !== null && r.env.toLowerCase().includes(q))
      );
    });
}

/** Per-kind count map; all kinds get a key (0 if absent) so the sidebar shows the full list. */
export function countsByKind(
  connections: Connection[],
): Partial<Record<ConnectionKind, number>> {
  const out: Partial<Record<ConnectionKind, number>> = {};
  for (const k of CONNECTION_KIND_ORDER) out[k] = 0;
  for (const c of connections) {
    const k = kindFromDriver(c.driver);
    if (k === null) continue;
    out[k] = (out[k] ?? 0) + 1;
  }
  return out;
}

/** Aggregate counts + status intent per environment. Production names get `warn`. */
export function envSummaries(enrichment: ConnectionEnrichment[]): EnvSummary[] {
  const counts = new Map<string, number>();
  for (const e of enrichment) {
    if (e.env === null) continue;
    counts.set(e.env, (counts.get(e.env) ?? 0) + 1);
  }
  return Array.from(counts.entries())
    .map(([name, count]) => ({
      name,
      count,
      status: isProductionName(name) ? ("warn" as const) : ("ok" as const),
    }))
    .sort((a, b) => a.name.localeCompare(b.name));
}

/** Aggregate total / ok / slow / down counts. Untested rows count toward total only. */
export function listStatusCounts(rows: ListRowItem[]): ListStatusCounts {
  let ok = 0;
  let slow = 0;
  let down = 0;
  for (const r of rows) {
    if (r.status === "ok") ok += 1;
    else if (r.status === "slow") slow += 1;
    else if (r.status === "down") down += 1;
  }
  return { total: rows.length, ok, slow, down };
}
