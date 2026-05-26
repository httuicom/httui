/**
 * Schema-aware SQL autocomplete for db blocks (extracted from
 * `cm-db-block.tsx` during A3 to keep that file under the 600 L cap).
 *
 * Behavior:
 *  - Off outside a db block body.
 *  - Off inside an active `{{ref}}` expression (ref autocomplete owns that).
 *  - After `FROM`/`JOIN`/`UPDATE`/`INTO` → tables.
 *  - After `table.` → columns of that table.
 *  - Elsewhere → tables + columns of tables already referenced in the body.
 *
 * Schema is pulled synchronously from the cache; on miss the store kicks
 * off an introspection in the background so the next keystroke has data.
 *
 * Public surface preserved (re-exported by `cm-db-block.tsx` so the test
 * import path `../cm-db-block` keeps working): `createDbSchemaCompletionSource`,
 * `__resetDbSchemaCompletionCache`.
 */

import type {
  Completion,
  CompletionContext,
  CompletionResult,
  CompletionSource,
} from "@codemirror/autocomplete";
import {
  PostgreSQL,
  MySQL,
  SQLite,
  StandardSQL,
  type SQLDialect,
} from "@codemirror/lang-sql";

import { resolveConnectionIdentifier } from "@/lib/blocks/connection-resolve";
import { listConnections, type Connection } from "@/lib/tauri/connections";
import { useSchemaCacheStore } from "@/stores/schemaCache";
import { findDbBlocks } from "@/lib/codemirror/cm-db-block";

// ───── Connections cache ─────

/**
 * Lazy connections cache for the completion source. Refreshed on cache miss
 * so newly-created connections become autocompleteable without a reload.
 */
let cachedConnections: Connection[] = [];
let connectionsPromise: Promise<Connection[]> | null = null;

async function ensureConnections(): Promise<Connection[]> {
  if (cachedConnections.length > 0) return cachedConnections;
  if (!connectionsPromise) {
    connectionsPromise = listConnections()
      .then((list) => {
        cachedConnections = list;
        return list;
      })
      .catch(() => {
        connectionsPromise = null;
        return [];
      });
  }
  return connectionsPromise;
}

// ───── Dialect / keyword resolution ─────

// Cached keyword lists per dialect so we don't re-split on every keystroke.
const KEYWORD_CACHE = new Map<string, Completion[]>();

/**
 * Resolve the effective SQL dialect. The fence keyword (`db-postgres` →
 * `"postgres"`) wins when explicit. For the bare `db` fence, fall back to
 * the connection driver so `db + sqlite connection` still ships SQLite
 * keywords instead of the empty StandardSQL bag.
 */
function effectiveDialect(
  fenceDialect: string | undefined,
  connectionDriver: string | undefined,
): string {
  const explicit =
    fenceDialect && fenceDialect !== "generic" ? fenceDialect : undefined;
  const driver = connectionDriver;
  return explicit ?? driver ?? "postgres";
}

function dialectFor(dialect: string): SQLDialect {
  switch (dialect) {
    case "postgres":
      return PostgreSQL;
    case "mysql":
      return MySQL;
    case "sqlite":
      return SQLite;
    default:
      return StandardSQL;
  }
}

function keywordsFor(dialect: string): Completion[] {
  const cached = KEYWORD_CACHE.get(dialect);
  if (cached) return cached;

  const spec = dialectFor(dialect).spec;
  const keywords = (spec.keywords ?? "")
    .split(/\s+/)
    .filter((k) => k.length > 0);
  const types = (spec.types ?? "").split(/\s+/).filter((k) => k.length > 0);
  const builtins = (spec.builtin ?? "")
    .split(/\s+/)
    .filter((k) => k.length > 0);

  const options: Completion[] = [
    ...keywords.map((label) => ({
      label,
      type: "keyword",
      detail: "keyword",
      boost: 1,
    })),
    ...types.map((label) => ({
      label,
      type: "type",
      detail: "type",
    })),
    ...builtins.map((label) => ({
      label,
      type: "variable",
      detail: "builtin",
    })),
  ];

  KEYWORD_CACHE.set(dialect, options);
  return options;
}

// ───── Public completion source ─────

export function createDbSchemaCompletionSource(): CompletionSource {
  return async (ctx: CompletionContext): Promise<CompletionResult | null> => {
    const pos = ctx.pos;
    const blocks = findDbBlocks(ctx.state.doc);
    const block = blocks.find((b) => pos >= b.bodyFrom && pos <= b.bodyTo);
    if (!block) return null;

    // Skip when inside a `{{ref}}` expression — let the ref source handle it.
    const bodyText = ctx.state.doc.sliceString(block.bodyFrom, block.bodyTo);
    const offsetInBody = pos - block.bodyFrom;
    const openIdx = bodyText.lastIndexOf("{{", offsetInBody);
    if (openIdx !== -1) {
      const closeIdx = bodyText.indexOf("}}", openIdx);
      if (closeIdx === -1 || closeIdx >= offsetInBody) return null;
    }

    const word = ctx.matchBefore(/[\w.]*/);
    if (!word || (word.from === word.to && !ctx.explicit)) return null;

    const text = word.text;

    // Resolve the connection (if any). Missing / orphan connections just
    // degrade the result set — we still offer SQL keywords so Ctrl-Space
    // never produces an empty popup inside a db block.
    const identifier = block.metadata.connection;
    const connection = identifier
      ? resolveConnectionIdentifier(await ensureConnections(), identifier)
      : null;

    // Dialect choice: fence wins when explicit (`db-postgres`), else the
    // connection driver (`db` + sqlite connection → SQLite keywords), else
    // PostgreSQL as a rich ANSI-ish fallback (avoids the empty StandardSQL).
    const dialect = effectiveDialect(
      block.metadata.dialect,
      connection?.driver,
    );

    const store = useSchemaCacheStore.getState();
    const schema = connection ? store.get(connection.id) : null;
    const schemaLoaded = !!schema;
    if (connection && !schema) {
      // Kick off lazy load so the next invocation has data.
      void store.ensureLoaded(connection.id);
    }

    // Build table map only when schema is ready.
    const tableMap: Record<string, string[]> = {};
    if (schema) {
      for (const table of schema.tables) {
        const key =
          table.schema && table.schema !== "public"
            ? `${table.schema}.${table.name}`
            : table.name;
        tableMap[key] = table.columns.map((c) => c.name);
      }
    }
    const tableNames = Object.keys(tableMap);

    // ── `<table-key>.` → columns of that table ──
    if (text.includes(".")) {
      const lastDot = text.lastIndexOf(".");
      const tableKey = text.slice(0, lastDot);
      const cols = tableMap[tableKey];
      if (cols && cols.length > 0) {
        return {
          from: word.from + lastDot + 1,
          to: word.to,
          options: cols.map((col) => ({
            label: col,
            type: "property",
            detail: tableKey,
          })),
          filter: true,
        };
      }
      // Unknown table prefix — fall through to keyword completion instead of
      // returning null (so Ctrl-Space after `some_alias.` still shows SQL).
    }

    const keywordOptions = keywordsFor(dialect);

    // ── After FROM/JOIN/UPDATE/INTO → tables first, then keywords ──
    const before = ctx.state.doc.sliceString(
      Math.max(block.bodyFrom, word.from - 32),
      word.from,
    );
    const prevKeyword = before.match(/\b(FROM|JOIN|UPDATE|INTO)\s+$/i);
    if (prevKeyword) {
      const tableOptions: Completion[] = tableNames.map((name) => ({
        label: name,
        type: "class",
        detail: `${tableMap[name].length} cols`,
        boost: 5,
      }));
      return {
        from: word.from,
        to: word.to,
        options: [
          ...tableOptions,
          ...statusHint(connection, identifier, schemaLoaded, ctx.explicit),
        ],
        filter: true,
      };
    }

    // ── Default: keywords + tables + columns of tables referenced in body ──
    const referenced = new Set<string>();
    const refRe =
      /\b(?:FROM|JOIN|UPDATE|INTO)\s+([A-Za-z_][\w]*(?:\.[A-Za-z_][\w]*)?)/gi;
    let m: RegExpExecArray | null;
    while ((m = refRe.exec(bodyText)) !== null) {
      if (tableMap[m[1]]) referenced.add(m[1]);
    }

    const columnOptions: Completion[] =
      referenced.size > 0
        ? [...referenced].flatMap((name) =>
            (tableMap[name] ?? []).map((col) => ({
              label: col,
              type: "property" as const,
              detail: name,
              boost: 3,
            })),
          )
        : [];

    const tableOptions: Completion[] = tableNames.map((name) => ({
      label: name,
      type: "class",
      detail: `${tableMap[name].length} cols`,
      boost: 2,
    }));

    const options: Completion[] = [
      ...columnOptions,
      ...tableOptions,
      ...keywordOptions,
      ...statusHint(connection, identifier, schemaLoaded, ctx.explicit),
    ];

    if (options.length === 0) return null;

    return {
      from: word.from,
      to: word.to,
      options,
      filter: true,
    };
  };
}

/**
 * Build a soft info-only completion that explains why tables are missing.
 * Rendered as a non-applicable "info row" so the user learns what to fix
 * without the option polluting the insertable list.
 *
 * Gated on `ctx.explicit`: these rows only appear when the user asked for
 * the popup with Ctrl-Space. When they're just typing, injecting a no-op
 * completion (`apply: () => {}`) causes CM6 to swallow Enter — the popup
 * eats the key trying to accept the top option, but the no-op apply
 * does nothing, and the user's newline never reaches the document.
 */
function statusHint(
  connection: Connection | null,
  identifier: string | undefined,
  schemaLoaded: boolean,
  explicit: boolean,
): Completion[] {
  if (!explicit) return [];
  if (!identifier) {
    return [
      {
        label: "⋯ no connection set",
        detail: "add `connection=<name>` to the fence",
        type: "text",
        boost: -99,
        apply: () => {}, // non-insertable
      },
    ];
  }
  if (!connection) {
    return [
      {
        label: `⋯ connection "${identifier}" not found`,
        detail: "check the schema panel for the correct name",
        type: "text",
        boost: -99,
        apply: () => {},
      },
    ];
  }
  if (!schemaLoaded) {
    return [
      {
        label: "⋯ loading schema",
        detail: "tables will appear shortly",
        type: "text",
        boost: -99,
        apply: () => {},
      },
    ];
  }
  return [];
}

/** Test/hot-reload hook — clears the module-level connections cache. */
export function __resetDbSchemaCompletionCache(): void {
  cachedConnections = [];
  connectionsPromise = null;
}
