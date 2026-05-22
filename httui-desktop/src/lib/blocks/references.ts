export interface Reference {
  raw: string;
  alias: string;
  path: string[];
  start: number;
  end: number;
}

export interface BlockContext {
  alias: string;
  blockType: string;
  pos: number;
  content: string;
  cachedResult: {
    status: string;
    response: string;
  } | null;
}

export interface ReferenceError {
  raw: string;
  message: string;
}

export interface ReferenceWarning {
  raw: string;
  message: string;
}

const REF_REGEX = /\{\{([^}]+)\}\}/g;

/** Property names that must not be accessed via JSON path navigation (prototype pollution defense). */
const DANGEROUS_KEYS = new Set(["__proto__", "constructor", "prototype"]);

/**
 * Extract all {{...}} references from text.
 */
export function parseReferences(text: string): Reference[] {
  const refs: Reference[] = [];
  let match: RegExpExecArray | null;
  REF_REGEX.lastIndex = 0;

  while ((match = REF_REGEX.exec(text)) !== null) {
    const inner = match[1].trim();
    const parts = inner.split(".");
    const alias = parts[0];
    const path = parts.slice(1);

    refs.push({
      raw: match[0],
      alias,
      path,
      start: match.index,
      end: match.index + match[0].length,
    });
  }

  return refs;
}

/**
 * Navigate a JSON value by dot-notation path.
 * Supports array indexing: "items.0.id"
 */
export function navigateJson(data: unknown, path: string[]): unknown {
  let current = data;

  for (const key of path) {
    if (current == null) {
      throw new Error(`Cannot access "${key}" on null/undefined`);
    }

    if (Array.isArray(current)) {
      const index = parseInt(key, 10);
      if (isNaN(index)) {
        throw new Error(`Expected numeric index for array, got "${key}"`);
      }
      if (index < 0 || index >= current.length) {
        throw new Error(
          `Index ${index} out of bounds (length: ${current.length})`,
        );
      }
      current = current[index];
    } else if (typeof current === "object") {
      if (DANGEROUS_KEYS.has(key)) {
        throw new Error(`Access to "${key}" is not allowed`);
      }
      const obj = current as Record<string, unknown>;
      if (!(key in obj)) {
        throw new Error(
          `Key "${key}" not found. Available: ${Object.keys(obj).join(", ")}`,
        );
      }
      current = obj[key];
    } else {
      throw new Error(`Cannot access "${key}" on ${typeof current}`);
    }
  }

  return current;
}

/** Positional alias: the executed block immediately above this one,
 * no explicit `alias=` needed. `{{$prev.body.id}}` ≈ the previous
 * block's response navigated by `.body.id` (response is the root —
 * the `.response` segment is implicit, unlike named refs). */
export const PREV_ALIAS = "$prev";

/** The block with the greatest `pos` strictly above `currentPos`. */
export function findPrevBlock(
  blocks: BlockContext[],
  currentPos: number,
): BlockContext | undefined {
  let prev: BlockContext | undefined;
  for (const b of blocks) {
    if (b.pos < currentPos && (!prev || b.pos > prev.pos)) prev = b;
  }
  return prev;
}

/**
 * Resolve a single reference against block contexts.
 */
export function resolveReference(
  ref: Reference,
  blocks: BlockContext[],
  currentPos: number,
): string {
  const isPrev = ref.alias === PREV_ALIAS;
  const block = isPrev
    ? findPrevBlock(blocks, currentPos)
    : blocks.find((b) => b.alias === ref.alias);

  if (!block) {
    throw new Error(
      isPrev
        ? `No previous block to reference with {{$prev}}`
        : `Alias "${ref.alias}" not found in document`,
    );
  }

  if (!isPrev && block.pos >= currentPos) {
    throw new Error(
      `Alias "${ref.alias}" is below current block (blocks can only reference blocks above)`,
    );
  }

  if (!block.cachedResult) {
    const label = isPrev ? "Previous block" : `Block "${ref.alias}"`;
    throw new Error(`${label} has no result yet — run it first.`);
  }

  let responseData: unknown;
  try {
    responseData = JSON.parse(block.cachedResult.response);
  } catch {
    const label = isPrev ? "Previous block" : `Alias "${ref.alias}"`;
    throw new Error(`${label} has invalid cached response`);
  }

  // For db blocks with the stage-2 response shape (`{results, messages,
  // stats}`), wrap the response in a view that supports two access patterns:
  //   - {{alias.response.0.rows.0.id}} → results[0].rows[0].id (explicit)
  //   - {{alias.response.id}}          → results[0].rows[0].id (legacy shim)
  // Pre-stage-2 cached shapes are navigated as-is so old caches keep working.
  const isDbBlock =
    block.blockType === "db" || block.blockType.startsWith("db-");
  const responseValue =
    isDbBlock && isNewDbResponseShape(responseData)
      ? makeDbResponseView(responseData)
      : responseData;

  // `$prev` roots navigation at the response itself (the `.response`
  // segment is implicit); named refs keep the `{response, status}`
  // envelope so `{{alias.response.x}}` / `{{alias.status}}` work.
  const root: unknown = isPrev
    ? responseValue
    : { response: responseValue, status: block.cachedResult.status };

  const value = navigateJson(root, ref.path);

  if (typeof value === "object" && value !== null) {
    return JSON.stringify(value);
  }
  return String(value);
}

function isNewDbResponseShape(value: unknown): value is { results: unknown[] } {
  if (!value || typeof value !== "object") return false;
  const obj = value as { results?: unknown };
  return Array.isArray(obj.results);
}

/**
 * Build a Proxy view over a stage-2 `DbResponse`. Three access patterns are
 * supported, all mapping onto the same underlying object:
 *   - Raw shape: `{{alias.response.results.0.rows.0.id}}` — passes through
 *     `results` / `messages` / `stats` to the DbResponse fields directly.
 *     This is what `{{` autocomplete guides users toward because it walks
 *     the raw JSON shape.
 *   - Numeric shortcut: `{{alias.response.0.rows.0.id}}` — `response.N`
 *     indexes into `results[N]`.
 *   - Legacy shim: `{{alias.response.id}}` — bare column names on the
 *     response delegate to `results[0].rows[0]` so pre-redesign refs
 *     keep resolving after the shape changed.
 */
const DB_RESPONSE_PASSTHROUGH_KEYS = new Set(["results", "messages", "stats"]);

function makeDbResponseView(dbResponse: { results: unknown[] }): object {
  const raw = dbResponse as Record<string, unknown>;

  const firstRow = (): Record<string, unknown> | null => {
    const first = dbResponse.results[0];
    if (!first || typeof first !== "object") return null;
    const rows = (first as { rows?: unknown }).rows;
    if (!Array.isArray(rows) || rows.length === 0) return null;
    const row = rows[0];
    if (!row || typeof row !== "object") return null;
    return row as Record<string, unknown>;
  };

  return new Proxy(
    {},
    {
      has(_target, key) {
        if (typeof key !== "string") return false;
        if (DB_RESPONSE_PASSTHROUGH_KEYS.has(key)) return key in raw;
        if (/^\d+$/.test(key)) {
          return parseInt(key, 10) < dbResponse.results.length;
        }
        const row = firstRow();
        return row !== null && key in row;
      },
      get(_target, key) {
        if (typeof key !== "string") return undefined;
        if (DB_RESPONSE_PASSTHROUGH_KEYS.has(key)) return raw[key];
        if (/^\d+$/.test(key)) {
          return dbResponse.results[parseInt(key, 10)];
        }
        const row = firstRow();
        return row ? row[key] : undefined;
      },
      ownKeys(_target) {
        const keys: string[] = [];
        for (const k of DB_RESPONSE_PASSTHROUGH_KEYS) {
          if (k in raw) keys.push(k);
        }
        for (let i = 0; i < dbResponse.results.length; i++)
          keys.push(String(i));
        const row = firstRow();
        if (row) keys.push(...Object.keys(row));
        return keys;
      },
      getOwnPropertyDescriptor(_target, _key) {
        return { enumerable: true, configurable: true };
      },
    },
  );
}

/**
 * Resolve all {{...}} references in text.
 * Returns resolved text and any errors (does not abort on first error).
 *
 * Resolution priority for each {{ref}}:
 * 1. Block reference (if a block with matching alias exists above) → use cached result
 * 2. Environment variable (if no block match and no dots) → use env value
 */
export function resolveAllReferences(
  text: string,
  blocks: BlockContext[],
  currentPos: number,
  envVariables?: Record<string, string>,
): {
  resolved: string;
  errors: ReferenceError[];
  warnings: ReferenceWarning[];
} {
  const refs = parseReferences(text);
  if (refs.length === 0) {
    return { resolved: text, errors: [], warnings: [] };
  }

  const errors: ReferenceError[] = [];
  const warnings: ReferenceWarning[] = [];
  let resolved = text;

  for (let i = refs.length - 1; i >= 0; i--) {
    const ref = refs[i];
    try {
      let value: string;

      // `$prev` is positional — never an env var; resolveReference
      // picks the previous block (and raises its own clear errors).
      if (ref.alias === PREV_ALIAS) {
        value = resolveReference(ref, blocks, currentPos);
        resolved =
          resolved.slice(0, ref.start) + value + resolved.slice(ref.end);
        continue;
      }

      const matchingBlock = blocks.find(
        (b) => b.alias === ref.alias && b.pos < currentPos,
      );
      if (matchingBlock) {
        // T37: Warn when block alias shadows an env var
        if (envVariables && ref.alias in envVariables) {
          warnings.push({
            raw: ref.raw,
            message: `Block alias "${ref.alias}" shadows environment variable with the same name`,
          });
        }
        value = resolveReference(ref, blocks, currentPos);
      } else if (
        ref.path.length === 0 &&
        envVariables &&
        ref.alias in envVariables
      ) {
        value = envVariables[ref.alias];
      } else {
        // No block, no env var — let resolveReference produce the proper error
        value = resolveReference(ref, blocks, currentPos);
      }

      resolved = resolved.slice(0, ref.start) + value + resolved.slice(ref.end);
    } catch (err) {
      errors.push({
        raw: ref.raw,
        message: err instanceof Error ? err.message : String(err),
      });
    }
  }

  return { resolved, errors, warnings };
}

/**
 * Convert `{{ref}}` placeholders in SQL (or any textual payload) to bind-
 * param markers (`?`) and collect the resolved values in order.
 *
 * Used by the DB block to safely pass user-referenced values into the
 * driver without string interpolation (SQL-safety is the whole point).
 *
 * - Resolution priority: block ref > env var (same as `resolveAllReferences`).
 * - Each resolved value is coerced: "true"/"false"/"null" become the
 *   literal JS types; numeric strings become numbers; everything else
 *   stays a string. The backend decides final typing per driver.
 * - On error the `{{…}}` is kept verbatim in the returned `sql` and the
 *   error is collected so callers can show a message.
 */
export function resolveRefsToBindParams(
  query: string,
  blocks: BlockContext[],
  currentPos: number,
  envVariables?: Record<string, string>,
): {
  sql: string;
  bindValues: unknown[];
  errors: string[];
  warnings: string[];
} {
  const refs = parseReferences(query);
  const bindValues: unknown[] = [];
  const errors: string[] = [];
  const warnings: string[] = [];

  let sql = query;

  for (let i = refs.length - 1; i >= 0; i--) {
    const ref = refs[i];
    const {
      resolved,
      errors: resolveErrors,
      warnings: resolveWarnings,
    } = resolveAllReferences(
      `{{${ref.raw.slice(2, -2).trim()}}}`,
      blocks,
      currentPos,
      envVariables,
    );

    for (const e of resolveErrors) errors.push(e.message);
    for (const w of resolveWarnings) warnings.push(w.message);

    if (resolveErrors.length > 0) {
      continue; // keep original in sql
    }

    // Coerce scalar strings into JS primitives.
    let value: unknown = resolved;
    if (resolved === "true") value = true;
    else if (resolved === "false") value = false;
    else if (resolved === "null") value = null;
    else if (resolved.trim() !== "") {
      const num = Number(resolved);
      if (!Number.isNaN(num) && String(num) === resolved.trim()) {
        value = num;
      }
    }

    bindValues.unshift(value);
    sql = sql.slice(0, ref.start) + "?" + sql.slice(ref.end);
  }

  return { sql, bindValues, errors, warnings };
}
