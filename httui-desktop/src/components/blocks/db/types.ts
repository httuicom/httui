export interface DbBlockData {
  connectionId: string;
  query: string;
  timeoutMs?: number;
}

export const DEFAULT_DB_DATA: DbBlockData = {
  connectionId: "",
  query: "",
};

export type CellValue =
  | string
  | number
  | boolean
  | null
  | CellValue[]
  | { [key: string]: CellValue };

export interface DbColumn {
  name: string;
  type: string;
}

export type DbRow = Record<string, CellValue>;

export type DbResult =
  | {
      kind: "select";
      columns: DbColumn[];
      rows: DbRow[];
      has_more: boolean;
    }
  | {
      kind: "mutation";
      rows_affected: number;
    }
  | {
      kind: "error";
      message: string;
      line?: number | null;
      column?: number | null;
    };

export type DbMessageSeverity = "notice" | "warning" | "error";

export interface DbMessage {
  severity: DbMessageSeverity;
  text: string;
  code?: string | null;
}

export interface DbStats {
  elapsed_ms: number;
  rows_streamed?: number | null;
}

export interface DbResponse {
  results: DbResult[];
  messages: DbMessage[];
  plan?: unknown;
  stats: DbStats;
}

export function isSelectResult(
  result: DbResult,
): result is Extract<DbResult, { kind: "select" }> {
  return result.kind === "select";
}

export function isMutationResult(
  result: DbResult,
): result is Extract<DbResult, { kind: "mutation" }> {
  return result.kind === "mutation";
}

export function isErrorResult(
  result: DbResult,
): result is Extract<DbResult, { kind: "error" }> {
  return result.kind === "error";
}

export function isDbResponse(value: unknown): value is DbResponse {
  if (!value || typeof value !== "object") return false;
  const obj = value as Record<string, unknown>;
  return Array.isArray(obj.results) && typeof obj.stats === "object";
}

// Legacy shapes kept only for cache/migration shim

interface LegacyDbColumn {
  name: string;
  type?: string;
  type_name?: string;
}

export interface LegacyDbSelectResponse {
  columns: LegacyDbColumn[];
  rows: DbRow[];
  has_more: boolean;
}

function coerceLegacyColumns(cols: LegacyDbColumn[]): DbColumn[] {
  return cols.map((c) => ({ name: c.name, type: c.type ?? c.type_name ?? "" }));
}

export interface LegacyDbMutationResponse {
  rows_affected: number;
}

export type LegacyDbResponse =
  | LegacyDbSelectResponse
  | LegacyDbMutationResponse;

function isLegacySelect(value: unknown): value is LegacyDbSelectResponse {
  if (!value || typeof value !== "object") return false;
  const obj = value as Record<string, unknown>;
  return Array.isArray(obj.columns) && Array.isArray(obj.rows);
}

function isLegacyMutation(value: unknown): value is LegacyDbMutationResponse {
  if (!value || typeof value !== "object") return false;
  const obj = value as Record<string, unknown>;
  return typeof obj.rows_affected === "number";
}

/**
 * Coerce any known DB response shape (new or legacy) into `DbResponse`.
 * Elapsed is `0` for legacy wrappers — the cache row's `duration_ms` is the
 * source of truth for elapsed when loading from an old cache entry.
 */
export function normalizeDbResponse(raw: unknown): DbResponse {
  if (isDbResponse(raw)) {
    return {
      results: raw.results,
      messages: Array.isArray(raw.messages) ? raw.messages : [],
      plan: raw.plan,
      stats: raw.stats,
    };
  }
  if (isLegacySelect(raw)) {
    return {
      results: [
        {
          kind: "select",
          columns: coerceLegacyColumns(raw.columns),
          rows: raw.rows,
          has_more: Boolean(raw.has_more),
        },
      ],
      messages: [],
      stats: { elapsed_ms: 0 },
    };
  }
  if (isLegacyMutation(raw)) {
    return {
      results: [{ kind: "mutation", rows_affected: raw.rows_affected }],
      messages: [],
      stats: { elapsed_ms: 0 },
    };
  }
  return { results: [], messages: [], stats: { elapsed_ms: 0 } };
}

/** Returns the first select result, or null if absent/non-select. */
export function firstSelectResult(
  response: DbResponse,
): Extract<DbResult, { kind: "select" }> | null {
  const first = response.results[0];
  return first && first.kind === "select" ? first : null;
}
