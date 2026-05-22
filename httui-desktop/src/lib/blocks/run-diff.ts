// Run diff.
//
// Pure side-by-side comparison between two runs of the same HTTP/DB
// block. Three diff layers: status (scalar), headers (key-level
// add/remove/change/equal), body (recursive JSON diff with path
// labels). Reorder of object keys is NOT a diff — JSON objects are
// unordered. Array diffing is index-aligned in V1 (per spec — "100ms
// for 100 KB bodies"); a smarter LCS pass can come later if needed.

const MAX_BODY_BYTES = 200 * 1024; // 200 KB per side per spec

export type JsonDiffOp = "add" | "remove" | "change";
export type HeaderDiffOp = "add" | "remove" | "change" | "equal";

export interface JsonDiffEntry {
  /** Dotted/bracketed path, e.g. `user.id` or `items[0].name`. */
  path: string;
  op: JsonDiffOp;
  before?: unknown;
  after?: unknown;
}

export interface HeaderDiffEntry {
  key: string;
  before?: string;
  after?: string;
  op: HeaderDiffOp;
}

export interface RunSnapshot {
  status?: number;
  headers?: Record<string, string>;
  body?: unknown;
  time_ms?: number;
  /** Approximate body size in bytes — when above MAX_BODY_BYTES, the
   * body diff is skipped and a single sentinel entry is returned. */
  size_bytes?: number;
}

export interface StatusDiff {
  before?: number;
  after?: number;
  changed: boolean;
}

export interface TimingDiff {
  before?: number;
  after?: number;
  deltaMs?: number;
}

export interface RunDiff {
  status: StatusDiff;
  headers: HeaderDiffEntry[];
  body: JsonDiffEntry[];
  timing: TimingDiff;
  /** True when at least one side's body exceeded MAX_BODY_BYTES and
   * the body diff was skipped. */
  bodyTruncated: boolean;
}

/** Recursive JSON diff with path-tagged entries. Unchanged keys
 * produce no entry; type mismatches produce a single `change`
 * entry at that path. */
export function diffJson(
  before: unknown,
  after: unknown,
  basePath = "",
): JsonDiffEntry[] {
  if (Object.is(before, after)) return [];

  if (Array.isArray(before) && Array.isArray(after)) {
    return diffArrays(before, after, basePath);
  }

  if (isPlainObject(before) && isPlainObject(after)) {
    return diffObjects(before, after, basePath);
  }

  // Scalars or mismatched types — emit a single change entry.
  if (before === undefined) {
    return [{ path: basePath || "$", op: "add", after }];
  }
  if (after === undefined) {
    return [{ path: basePath || "$", op: "remove", before }];
  }
  return [{ path: basePath || "$", op: "change", before, after }];
}

function diffObjects(
  before: Record<string, unknown>,
  after: Record<string, unknown>,
  basePath: string,
): JsonDiffEntry[] {
  const out: JsonDiffEntry[] = [];
  const keys = new Set([...Object.keys(before), ...Object.keys(after)]);
  for (const key of [...keys].sort()) {
    const path = basePath ? `${basePath}.${key}` : key;
    const inBefore = key in before;
    const inAfter = key in after;
    if (inBefore && !inAfter) {
      out.push({ path, op: "remove", before: before[key] });
      continue;
    }
    if (!inBefore && inAfter) {
      out.push({ path, op: "add", after: after[key] });
      continue;
    }
    out.push(...diffJson(before[key], after[key], path));
  }
  return out;
}

function diffArrays(
  before: unknown[],
  after: unknown[],
  basePath: string,
): JsonDiffEntry[] {
  const out: JsonDiffEntry[] = [];
  const max = Math.max(before.length, after.length);
  for (let i = 0; i < max; i++) {
    const path = `${basePath}[${i}]`;
    if (i >= before.length) {
      out.push({ path, op: "add", after: after[i] });
      continue;
    }
    if (i >= after.length) {
      out.push({ path, op: "remove", before: before[i] });
      continue;
    }
    out.push(...diffJson(before[i], after[i], path));
  }
  return out;
}

/** Header diff — case-insensitive key match. Returns one entry per
 * union key, with op `add`/`remove`/`change`/`equal`. The display
 * key uses the casing from `after` when present, else `before`. */
export function diffHeaders(
  before: Readonly<Record<string, string>> = {},
  after: Readonly<Record<string, string>> = {},
): HeaderDiffEntry[] {
  const beforeMap = lowerMap(before);
  const afterMap = lowerMap(after);
  const lowerKeys = new Set([
    ...Object.keys(beforeMap),
    ...Object.keys(afterMap),
  ]);
  const out: HeaderDiffEntry[] = [];
  for (const lk of [...lowerKeys].sort()) {
    const beforeEntry = beforeMap[lk];
    const afterEntry = afterMap[lk];
    if (beforeEntry && !afterEntry) {
      out.push({
        key: beforeEntry.original,
        op: "remove",
        before: beforeEntry.value,
      });
      continue;
    }
    if (!beforeEntry && afterEntry) {
      out.push({
        key: afterEntry.original,
        op: "add",
        after: afterEntry.value,
      });
      continue;
    }
    if (beforeEntry && afterEntry) {
      const op = beforeEntry.value === afterEntry.value ? "equal" : "change";
      out.push({
        key: afterEntry.original,
        op,
        before: beforeEntry.value,
        after: afterEntry.value,
      });
    }
  }
  return out;
}

/** Top-level run-vs-run diff. Skips body diffing when either side
 * exceeds the 200 KB cap (per spec — "degrades gracefully past
 * 200 KB"). */
export function diffRuns(runA: RunSnapshot, runB: RunSnapshot): RunDiff {
  const status: StatusDiff = {
    before: runA.status,
    after: runB.status,
    changed: runA.status !== runB.status,
  };

  const timing: TimingDiff = {
    before: runA.time_ms,
    after: runB.time_ms,
    deltaMs:
      runA.time_ms !== undefined && runB.time_ms !== undefined
        ? runB.time_ms - runA.time_ms
        : undefined,
  };

  const headers = diffHeaders(runA.headers, runB.headers);

  const aOversize = (runA.size_bytes ?? 0) > MAX_BODY_BYTES;
  const bOversize = (runB.size_bytes ?? 0) > MAX_BODY_BYTES;
  const bodyTruncated = aOversize || bOversize;
  const body = bodyTruncated ? [] : diffJson(runA.body, runB.body);

  return { status, headers, body, timing, bodyTruncated };
}

function isPlainObject(v: unknown): v is Record<string, unknown> {
  return (
    typeof v === "object" &&
    v !== null &&
    !Array.isArray(v) &&
    Object.getPrototypeOf(v) === Object.prototype
  );
}

interface LowerEntry {
  original: string;
  value: string;
}

function lowerMap(
  obj: Readonly<Record<string, string>>,
): Record<string, LowerEntry> {
  const out: Record<string, LowerEntry> = {};
  for (const [k, v] of Object.entries(obj)) {
    out[k.toLowerCase()] = { original: k, value: v };
  }
  return out;
}
