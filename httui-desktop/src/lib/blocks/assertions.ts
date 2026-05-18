// Block assertions.
//
// Pure parser + evaluator for the canvas §10 inline-test syntax. The
// `# expect:` marker section lives inside the fenced HTTP/DB block
// after the body; lines like `# <lhs> <op> <rhs>` describe one
// expectation each. Lines that don't parse are silently dropped (per
// the spec — the user shouldn't have to escape every comment).
//
// Storage example:
//   ```http alias=getUser
//   GET https://api.example.com/users/1
//
//   # expect:
//   # status === 200
//   # time < 1000
//   # $.body.id === 1
//   ```

const EXPECT_MARKER = /^\s*#\s*expect\s*:\s*$/i;
const COMMENT_PREFIX = /^\s*#\s?/;
const NUMBER_LITERAL = /^-?\d+(?:\.\d+)?$/;
const STRING_LITERAL = /^(?:"((?:\\.|[^"\\])*)"|'((?:\\.|[^'\\])*)')$/;
const REGEX_LITERAL = /^\/((?:\\.|[^/\\])+)\/([gimsuy]*)$/;
const OP_TOKENS = [
  "===",
  "!==",
  "<=",
  ">=",
  "<",
  ">",
  "matches",
  "contains",
] as const;

export type AssertionOp = (typeof OP_TOKENS)[number];

export interface ParsedAssertion {
  /** 1-indexed line offset *within the block body* — useful for
   * surfacing failures back to the editor. */
  line: number;
  /** Original line (after stripping the `#` comment prefix). */
  raw: string;
  lhs: string;
  op: AssertionOp;
  rhs: string;
}

export interface AssertionContext {
  status?: number;
  /** Total elapsed milliseconds. */
  time_ms?: number;
  body?: unknown;
  /** HTTP response headers; lookups are case-insensitive. */
  headers?: Record<string, string>;
  /** DB result rows; `$.row[N].col` indexes here. */
  row?: ReadonlyArray<Record<string, unknown>>;
}

export interface AssertionFailure {
  line: number;
  raw: string;
  actual: unknown;
  expected: unknown;
  reason?: string;
}

export interface AssertionResult {
  pass: boolean;
  failures: AssertionFailure[];
}

/** Walk the block body and return the lines under `# expect:` (after
 * stripping the comment prefix). Returns `[]` when no marker is
 * present. Stops at the first blank or EOB. */
export function extractAssertionLines(blockBody: string): {
  rawLine: string;
  bodyLine: number;
}[] {
  const lines = blockBody.split("\n");
  const out: { rawLine: string; bodyLine: number }[] = [];
  let inSection = false;
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (!inSection) {
      if (EXPECT_MARKER.test(line)) inSection = true;
      continue;
    }
    if (line.trim() === "") break;
    if (!COMMENT_PREFIX.test(line)) break;
    out.push({
      rawLine: line.replace(COMMENT_PREFIX, ""),
      bodyLine: i + 1,
    });
  }
  return out;
}

/** Tokenize one assertion line into `{ lhs, op, rhs }`. Returns null
 * when the line doesn't fit `<lhs> <op> <rhs>`. */
export function parseAssertionLine(
  rawLine: string,
  bodyLine: number,
): ParsedAssertion | null {
  const trimmed = rawLine.trim();
  if (!trimmed) return null;
  for (const op of OP_TOKENS) {
    const idx = findOperator(trimmed, op);
    if (idx < 0) continue;
    const lhs = trimmed.slice(0, idx).trim();
    const rhs = trimmed.slice(idx + op.length).trim();
    if (!lhs || !rhs) return null;
    return { line: bodyLine, raw: trimmed, lhs, op, rhs };
  }
  return null;
}

function findOperator(line: string, op: AssertionOp): number {
  if (op === "matches" || op === "contains") {
    const re = new RegExp(`\\s${op}\\s`);
    const match = re.exec(line);
    return match ? match.index + 1 : -1;
  }
  return line.indexOf(op);
}

export function parseAllAssertions(blockBody: string): ParsedAssertion[] {
  const lines = extractAssertionLines(blockBody);
  const out: ParsedAssertion[] = [];
  for (const { rawLine, bodyLine } of lines) {
    const parsed = parseAssertionLine(rawLine, bodyLine);
    if (parsed) out.push(parsed);
  }
  return out;
}

/** Resolve an LHS expression into a value pulled from `ctx`. Returns
 * `undefined` when the path doesn't exist. */
export function resolveLhs(lhs: string, ctx: AssertionContext): unknown {
  const trimmed = lhs.trim();
  if (trimmed === "status") return ctx.status;
  if (trimmed === "time") return ctx.time_ms;
  if (trimmed.startsWith("$.headers.")) {
    const name = trimmed.slice("$.headers.".length).toLowerCase();
    if (!ctx.headers) return undefined;
    for (const [k, v] of Object.entries(ctx.headers)) {
      if (k.toLowerCase() === name) return v;
    }
    return undefined;
  }
  if (trimmed.startsWith("$.body")) {
    return navigatePath(ctx.body, trimmed.slice("$.body".length));
  }
  if (trimmed.startsWith("$.row")) {
    return navigatePath(ctx.row, trimmed.slice("$.row".length));
  }
  return undefined;
}

/** Coerce `<rhs>` token into a typed value: number / string / regex.
 * Returns `{ kind: "raw", value }` for tokens that don't match a
 * literal — those are compared as strings. */
export function parseRhs(
  rhs: string,
):
  | { kind: "number"; value: number }
  | { kind: "string"; value: string }
  | { kind: "regex"; value: RegExp }
  | { kind: "raw"; value: string } {
  const trimmed = rhs.trim();
  if (NUMBER_LITERAL.test(trimmed)) {
    return { kind: "number", value: Number(trimmed) };
  }
  const strMatch = STRING_LITERAL.exec(trimmed);
  if (strMatch) {
    return { kind: "string", value: strMatch[1] ?? strMatch[2] ?? "" };
  }
  const reMatch = REGEX_LITERAL.exec(trimmed);
  if (reMatch) {
    try {
      return { kind: "regex", value: new RegExp(reMatch[1], reMatch[2]) };
    } catch {
      // fall through to raw
    }
  }
  return { kind: "raw", value: trimmed };
}

/** Evaluate a single parsed assertion against `ctx`. Returns
 * `{ pass: true }` on match, `{ pass: false, failure }` on miss. */
export function evaluateAssertion(
  parsed: ParsedAssertion,
  ctx: AssertionContext,
): { pass: true } | { pass: false; failure: AssertionFailure } {
  const actual = resolveLhs(parsed.lhs, ctx);
  const rhs = parseRhs(parsed.rhs);
  const expected =
    rhs.kind === "regex"
      ? `/${rhs.value.source}/${rhs.value.flags}`
      : rhs.value;
  const reason = compareReason(parsed.op, actual, rhs);
  if (reason === null) return { pass: true };
  return {
    pass: false,
    failure: {
      line: parsed.line,
      raw: parsed.raw,
      actual,
      expected,
      reason,
    },
  };
}

type RhsParsed = ReturnType<typeof parseRhs>;

function compareReason(
  op: AssertionOp,
  actual: unknown,
  rhs: RhsParsed,
): string | null {
  switch (op) {
    case "===":
      return strictEqual(actual, rhs) ? null : "values not equal";
    case "!==":
      return strictEqual(actual, rhs) ? "values are equal" : null;
    case "<":
    case "<=":
    case ">":
    case ">=":
      return numericCompare(op, actual, rhs);
    case "matches":
      return matchesCompare(actual, rhs);
    case "contains":
      return containsCompare(actual, rhs);
  }
}

function strictEqual(actual: unknown, rhs: RhsParsed): boolean {
  if (rhs.kind === "regex") {
    return typeof actual === "string" && rhs.value.test(actual);
  }
  if (rhs.kind === "number") return actual === rhs.value;
  if (rhs.kind === "string") return actual === rhs.value;
  // raw fallback: stringify and compare
  return String(actual) === rhs.value;
}

function numericCompare(
  op: AssertionOp,
  actual: unknown,
  rhs: RhsParsed,
): string | null {
  if (rhs.kind !== "number")
    return `rhs not a number: ${JSON.stringify(rhs.value)}`;
  if (typeof actual !== "number")
    return `actual not a number: ${JSON.stringify(actual)}`;
  const ok =
    op === "<"
      ? actual < rhs.value
      : op === "<="
        ? actual <= rhs.value
        : op === ">"
          ? actual > rhs.value
          : actual >= rhs.value;
  return ok ? null : `${actual} ${op} ${rhs.value} is false`;
}

function matchesCompare(actual: unknown, rhs: RhsParsed): string | null {
  if (rhs.kind !== "regex") return "rhs is not a regex literal /.../";
  if (typeof actual !== "string") return "actual is not a string";
  return rhs.value.test(actual) ? null : "regex did not match";
}

function containsCompare(actual: unknown, rhs: RhsParsed): string | null {
  const needle = rhs.kind === "string" || rhs.kind === "raw" ? rhs.value : null;
  if (typeof actual === "string") {
    if (needle === null) return "rhs not stringy";
    return actual.includes(needle) ? null : "substring not found";
  }
  if (Array.isArray(actual)) {
    const target = rhs.kind === "number" ? rhs.value : needle;
    if (target === null) return "rhs not comparable";
    return actual.some((el) => el === target) ? null : "element not found";
  }
  return "actual is neither string nor array";
}

export function evaluateAllAssertions(
  parsed: ReadonlyArray<ParsedAssertion>,
  ctx: AssertionContext,
): AssertionResult {
  const failures: AssertionFailure[] = [];
  for (const p of parsed) {
    const result = evaluateAssertion(p, ctx);
    if (!result.pass) failures.push(result.failure);
  }
  return { pass: failures.length === 0, failures };
}

// --- Adapters from concrete response shapes (runtime wiring) --------------

/** Build an `AssertionContext` from the HTTP response shape emitted by
 * `executeHttpStreamed`. `time_ms` prefers `timing.total_ms` over
 * `elapsed_ms` because the breakdown total is the canonical figure;
 * either is fine for the `time < N` predicate. */
export function httpResponseToAssertionContext(resp: {
  status_code: number;
  headers: Record<string, string>;
  body: unknown;
  elapsed_ms?: number;
  timing?: { total_ms?: number };
}): AssertionContext {
  return {
    status: resp.status_code,
    time_ms: resp.timing?.total_ms ?? resp.elapsed_ms,
    body: resp.body,
    headers: resp.headers,
  };
}

/** Build an `AssertionContext` from the DB response shape. Uses the
 * first SELECT result's rows for `$.row[N].col`; mutation-only or
 * error-only responses produce an empty `row` array (so `$.row[N]`
 * evaluates to undefined and assertions fail with a clear reason). */
export function dbResponseToAssertionContext(resp: {
  results: ReadonlyArray<{ kind: string } & Record<string, unknown>>;
  stats?: { elapsed_ms?: number };
}): AssertionContext {
  const select = resp.results.find((r) => r.kind === "select");
  const rows =
    select && Array.isArray(select.rows)
      ? (select.rows as Record<string, unknown>[])
      : [];
  return {
    time_ms: resp.stats?.elapsed_ms,
    row: rows,
    body: rows,
  };
}

/** Navigate a JSONPath fragment like `.foo.bar` or `[0].col`. Returns
 * undefined when the path doesn't resolve. Limited subset: dot
 * descent and numeric index brackets only — no filters, no `..`. */
function navigatePath(root: unknown, path: string): unknown {
  if (!path) return root;
  let current: unknown = root;
  let i = 0;
  while (i < path.length) {
    const c = path[i];
    if (c === ".") {
      i++;
      const start = i;
      while (i < path.length && path[i] !== "." && path[i] !== "[") i++;
      const key = path.slice(start, i);
      if (!key) return undefined;
      if (current == null || typeof current !== "object") return undefined;
      current = (current as Record<string, unknown>)[key];
      continue;
    }
    if (c === "[") {
      const close = path.indexOf("]", i + 1);
      if (close < 0) return undefined;
      const idx = Number(path.slice(i + 1, close));
      if (!Number.isInteger(idx)) return undefined;
      if (!Array.isArray(current)) return undefined;
      current = current[idx];
      i = close + 1;
      continue;
    }
    return undefined;
  }
  return current;
}
