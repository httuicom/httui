/**
 * Block-type registry — CM-level half (A5, audit 03 #2 / §"target
 * architecture"). Each `BlockTypeSpec` describes a fenced-block type's
 * editor-facing surface: id/label, slash commands, Lucide icon SVGs,
 * the CM6 extension factory, and its autocomplete sources.
 *
 * Consumed by `markdown-extensions.ts` (iterates `createExtension` +
 * `completionSources`) and `cm-slash-commands.ts` (iterates
 * `slashCommands` + `icons`). After A5: adding a new block type =
 * create its module file (cm-*-block + Panel + Fenced{Block,Spec}) +
 * register here. **Zero edits** to markdown-extensions / cm-slash-
 * commands / MarkdownEditor.
 *
 * The React/Panel half lives in `block-portal-registry.tsx` to keep
 * heavy React deps off the editor-extension import path (the
 * `markdown-extensions.test.ts` mocks the CM modules but not the
 * panels — splitting prevents pulling HttpFencedPanel/DbFencedPanel
 * trees through that test).
 */

import type { Extension } from "@codemirror/state";
import type {
  CompletionSection,
  CompletionSource,
} from "@codemirror/autocomplete";

import {
  createDbBlockExtension,
  createDbBlockCompletionSource,
  createDbSchemaCompletionSource,
} from "@/lib/codemirror/cm-db-block";
import {
  createHttpBlockExtension,
  createHttpBlockCompletionSource,
} from "@/lib/codemirror/cm-http-block";

/**
 * Mirror of the slash-command shape lived inline in `cm-slash-commands
 * .ts`. Section is supplied here (always `EXEC` for block types) but
 * `cm-slash-commands` resolves it against its own section constants.
 */
export interface BlockSlashCommand {
  label: string;
  /** Icon type key — must match an entry in `BlockTypeSpec.icons`. */
  type: string;
  insert: string;
  cursorOffset?: number;
  shortcut?: string;
}

/** Lucide SVG inner paths registered by a block type (no `<svg>` wrapper). */
export interface BlockIconSpec {
  /** Type key referenced by `BlockSlashCommand.type` and the SVG lookup. */
  type: string;
  /** Concatenated `<path>` / `<line>` / `<rect>` markup, Lucide style. */
  paths: string;
}

export interface BlockTypeSpec {
  /** Stable id — "http" | "db" | future. */
  id: string;
  /** User-facing badge text. */
  label: string;
  /** Lucide SVG paths this block type contributes. Usually 1; can be more. */
  icons: BlockIconSpec[];
  /** Slash menu entries injected into the "Executable" section. */
  slashCommands: BlockSlashCommand[];
  /** CM6 extension (scanner + decorations + keymap + state field). */
  createExtension: () => Extension;
  /**
   * Autocomplete sources contributed inside this block type's body —
   * usually 1 ({{ref}}); DB returns 2 ({{ref}} + schema-aware SQL).
   * Order matters: ref-completion FIRST so it owns `{{...` regions.
   */
  completionSources: (
    getFilePath: () => string | undefined,
  ) => CompletionSource[];
}

// ── Block type modules ────────────────────────────────────────────

const dbBlock: BlockTypeSpec = {
  id: "db",
  label: "DB",
  icons: [
    {
      type: "database",
      paths:
        '<ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M3 5V19A9 3 0 0 0 21 19V5"/><path d="M3 12A9 3 0 0 0 21 12"/>',
    },
  ],
  slashCommands: [
    // DB blocks use the post-redesign fenced-SQL format (see
    // docs/db-block-redesign.md §2.1). cursorOffset lands the caret on
    // the empty body line so the user can start typing SQL immediately;
    // the drawer (⚙) is the preferred way to pick a connection, so we
    // leave `connection=` off the canonical info string by default.
    {
      label: "PostgreSQL Query",
      type: "database",
      insert: "```db-postgres alias=db1\n\n```\n",
      cursorOffset: -5,
    },
    {
      label: "MySQL Query",
      type: "database",
      insert: "```db-mysql alias=db1\n\n```\n",
      cursorOffset: -5,
    },
    {
      label: "SQLite Query",
      type: "database",
      insert: "```db-sqlite alias=db1\n\n```\n",
      cursorOffset: -5,
    },
  ],
  createExtension: createDbBlockExtension,
  completionSources: (getFilePath) => [
    // {{ref}} autocomplete — gated to db-* fenced body.
    createDbBlockCompletionSource(getFilePath),
    // Schema-aware SQL — same gating; reads from the SchemaCache store.
    createDbSchemaCompletionSource(),
  ],
};

const httpBlock: BlockTypeSpec = {
  id: "http",
  label: "HTTP",
  icons: [
    {
      type: "http",
      paths:
        '<path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>',
    },
  ],
  slashCommands: [
    // HTTP blocks use the post-redesign HTTP-message body format (see
    // docs/http-block-redesign.md §2.1). cursorOffset lands the caret
    // on the request line so the user can start typing immediately.
    {
      label: "HTTP Request",
      type: "http",
      insert: "```http alias=req1\nGET \n```\n",
      cursorOffset: -5,
    },
    {
      label: "HTTP GET",
      type: "http",
      insert: "```http alias=req1\nGET \n```\n",
      cursorOffset: -5,
    },
    {
      label: "HTTP POST",
      type: "http",
      insert:
        "```http alias=req1\nPOST \nContent-Type: application/json\n\n{}\n```\n",
      cursorOffset: -23,
    },
    {
      label: "HTTP PUT",
      type: "http",
      insert:
        "```http alias=req1\nPUT \nContent-Type: application/json\n\n{}\n```\n",
      cursorOffset: -23,
    },
    {
      label: "HTTP DELETE",
      type: "http",
      insert: "```http alias=req1\nDELETE \n```\n",
      cursorOffset: -5,
    },
  ],
  createExtension: createHttpBlockExtension,
  completionSources: (getFilePath) => [
    createHttpBlockCompletionSource(getFilePath),
  ],
};

/**
 * Ordered list of registered block types. Editor composition iterates
 * this; the order is observable (extension priority + slash menu
 * grouping). DB before HTTP preserves the pre-A5 sequence.
 */
export const blockRegistry: BlockTypeSpec[] = [dbBlock, httpBlock];

/**
 * Section-aware flat list of slash commands contributed by every
 * registered block. Consumed by `cm-slash-commands.ts` to splice into
 * the global COMMANDS array under the "Executable" section. The
 * section is wired here so the registry doesn't need to know about
 * `CompletionSection` instances from cm-slash-commands.
 */
export function getRegisteredBlockSlashCommands(
  execSection: CompletionSection,
): Array<BlockSlashCommand & { section: CompletionSection }> {
  return blockRegistry.flatMap((m) =>
    m.slashCommands.map((s) => ({ ...s, section: execSection })),
  );
}

/** Flat icon map every registered block contributes. Keyed by `type`. */
export function getRegisteredBlockIcons(): Record<string, string> {
  const map: Record<string, string> = {};
  for (const m of blockRegistry) {
    for (const icon of m.icons) {
      map[icon.type] = icon.paths;
    }
  }
  return map;
}
