/**
 * CodeMirror extension for DB block rendering (stages 4-5 of db block
 * redesign).
 *
 * Post-A3: the generic skeleton (scanner, registry, decorations, keymap,
 * StateField, ref autocomplete) lives in `widget-portal-registry.ts` +
 * `createFencedBlockExtension.tsx`. The schema-aware SQL autocomplete is
 * in the sibling `cm-db-schema-complete.ts` (extracted to keep this file
 * under the 600 L cap; re-exported below so the legacy import path holds).
 * This file owns only: the open-fence regex (with the dialect group),
 * the body decoration, the SQL error-mark machinery (`setDbBlockErrors`),
 * and the public surface stitching.
 *
 * Public exports are preserved exactly (RULE 4): `findDbBlocks`,
 * `createDbBlockExtension`, `createDbBlockCompletionSource`,
 * `createDbSchemaCompletionSource`, `subscribeToDbPortals`,
 * `getDbPortalVersion`, `getDbWidgetContainers`, `setDbBlockActions`,
 * `setDbBlockErrors`, the `Db*` types, `__internal`, and
 * `__resetDbSchemaCompletionCache` — consumers + tests rely on these.
 */

import {
  EditorState,
  RangeSetBuilder,
  StateEffect,
  StateField,
  type Extension,
  type Text as CMText,
} from "@codemirror/state";
import { Decoration, EditorView, type DecorationSet } from "@codemirror/view";
import type { CompletionSource } from "@codemirror/autocomplete";

import { parseDbFenceInfo, type DbBlockMetadata } from "@/lib/blocks/db-fence";
import {
  WidgetPortalRegistry,
  type FencedBlockBase,
  type PortalEntryOf,
} from "@/lib/codemirror/widget-portal-registry";
import {
  buildFenceDecorations,
  createFencedBlockExtension,
  FENCE_CLOSE_RE,
  makeFencedScanner,
  makeRefCompletionSource,
  type PushItem,
} from "@/lib/codemirror/createFencedBlockExtension";

// Re-exported (test + markdown-extensions import them from this module).
export {
  createDbSchemaCompletionSource,
  __resetDbSchemaCompletionCache,
} from "@/lib/codemirror/cm-db-schema-complete";

// ───── Types ─────

export interface DbFencedBlock extends FencedBlockBase {
  lang: string;
  metadata: DbBlockMetadata;
}

export type DbWidgetSlot = "toolbar" | "result" | "statusbar";

export interface DbPortalActions {
  /** Run the block. Called by ⌘↵ or the toolbar ▶ button. */
  onRun?: () => void;
  /** Cancel an in-flight run. Called by ⌘. or the toolbar ⏹ button. */
  onCancel?: () => void;
  /** Open the settings drawer. Called by the ⚙ button. */
  onOpenSettings?: () => void;
  /** Run the query wrapped in EXPLAIN. Called by ⌘⇧E or the ▦ button. */
  onExplain?: () => void;
}

export type DbPortalEntry = PortalEntryOf<
  DbWidgetSlot,
  DbPortalActions,
  DbFencedBlock
>;

const DB_OPEN_RE = /^```(db(?:-[\w:-]+)?)(.*)$/;

// ───── Scanner ─────

const scanner = makeFencedScanner<
  DbFencedBlock,
  Pick<DbFencedBlock, "lang" | "info" | "metadata">
>({
  openRe: DB_OPEN_RE,
  parse: (match) => {
    const lang = match[1];
    const info = match[2].trim();
    const metadata = parseDbFenceInfo(`${lang} ${info}`.trim()) ?? {
      dialect: "generic" as const,
    };
    return { lang, info, metadata };
  },
});

export function findDbBlocks(doc: CMText): DbFencedBlock[] {
  return scanner.findBlocks(doc);
}

// ───── Registry (module-level singleton — preserved behavior) ─────

const registry = new WidgetPortalRegistry<
  DbWidgetSlot,
  DbPortalActions,
  DbFencedBlock
>({
  idPrefix: "db_idx_",
  slots: ["toolbar", "result", "statusbar"],
  metaChanged: (prev, next) =>
    prev.metadata.dialect !== next.metadata.dialect ||
    prev.metadata.alias !== next.metadata.alias ||
    prev.metadata.connection !== next.metadata.connection ||
    prev.metadata.limit !== next.metadata.limit ||
    prev.metadata.timeoutMs !== next.metadata.timeoutMs ||
    prev.metadata.displayMode !== next.metadata.displayMode,
  // 250ms debounce — we don't want React to re-render the whole panel on
  // every keystroke in the query. DB has no form view, so the form-flash
  // bug that forced HTTP to drop this debounce doesn't apply here.
  bodyChangePolicy: "debounced",
  dedupeSameSlotElement: false,
});

export const subscribeToDbPortals = registry.subscribe;
export const getDbPortalVersion = registry.getVersion;
export const getDbWidgetContainers = registry.getContainers;
export const setDbBlockActions = registry.setBlockActions;

// ───── Widgets (generic factories instantiated with DB class names) ─────

const DbToolbarPortalWidget = registry.slotWidget(
  "toolbar",
  "cm-db-toolbar-portal",
  44,
);

const DbClosePanelWidget = registry.closePanelWidget({
  wrapClass: "cm-db-close-panel",
  spacerClass: "cm-db-fence-hidden",
  resultClass: "cm-db-result-portal",
  statusClass: "cm-db-statusbar-portal",
  resultSlot: "result",
  statusSlot: "statusbar",
  // empty-state-ish default (result tabs + status)
  fallbackHeight: 120,
});

// ───── DB-specific body decoration ─────

/**
 * Body lines: just per-line classes. No form view, no syntax classification,
 * no method coloring — DB's body is plain SQL handled by the SQL language
 * highlighter elsewhere in the editor stack. First/last get modifier
 * classes so only the outer edges round.
 */
function decorateDbBody(
  state: EditorState,
  block: DbFencedBlock,
  _blockId: string,
  editing: boolean,
  push: PushItem,
): void {
  if (block.body.length === 0) return;
  const firstBodyLine = state.doc.lineAt(block.bodyFrom).number;
  const lastBodyLine = state.doc.lineAt(block.bodyTo).number;
  for (let n = firstBodyLine; n <= lastBodyLine; n++) {
    const line = state.doc.line(n);
    const classes = ["cm-db-body-line"];
    if (editing) classes.push("cm-db-body-editing");
    if (n === firstBodyLine) classes.push("cm-db-body-line-first");
    if (n === lastBodyLine) classes.push("cm-db-body-line-last");
    push({
      from: line.from,
      to: line.from,
      deco: Decoration.line({ class: classes.join(" ") }),
      order: 0,
    });
  }
}

// ───── Error squiggle (stage 8b — DB-only) ─────

/**
 * A SQL error reported by the backend, anchored at (line, column) inside
 * the block body. `length` is how many characters the squiggle covers —
 * defaults to the token starting at that position, capped at 32 chars so
 * a runaway position doesn't underline the rest of the block.
 */
export interface DbErrorMark {
  blockId: string;
  line: number;
  column: number;
  length?: number;
  message?: string;
}

const setDbErrorsEffect = StateEffect.define<DbErrorMark[]>();
const clearDbErrorsEffect = StateEffect.define<string>();

/**
 * State field: map of `blockId` → current error marks. Rebuilt only when
 * an effect fires, so keystrokes don't invalidate it.
 */
const dbErrorsField = StateField.define<Map<string, DbErrorMark[]>>({
  create: () => new Map(),
  update(value, tr) {
    let next = value;
    for (const effect of tr.effects) {
      if (effect.is(setDbErrorsEffect)) {
        next = new Map(next);
        const marks = effect.value;
        if (marks.length === 0) continue;
        // All marks from a single dispatch share a blockId (the panel
        // sends per-block updates); group defensively anyway.
        const byBlock = new Map<string, DbErrorMark[]>();
        for (const m of marks) {
          const list = byBlock.get(m.blockId) ?? [];
          list.push(m);
          byBlock.set(m.blockId, list);
        }
        for (const [id, list] of byBlock) {
          next.set(id, list);
        }
      } else if (effect.is(clearDbErrorsEffect)) {
        if (next.has(effect.value)) {
          next = new Map(next);
          next.delete(effect.value);
        }
      }
    }
    return next;
  },
});

/**
 * Convert a (line, column) inside a block body into an absolute doc
 * offset. Both inputs are 1-indexed (following Postgres / MySQL). Returns
 * null when the coordinates overrun the body.
 */
function bodyLineColToOffset(
  doc: CMText,
  block: DbFencedBlock,
  line: number,
  column: number,
): number | null {
  if (line < 1 || column < 1) return null;
  let offset = block.bodyFrom;
  let lineCount = 1;
  while (lineCount < line && offset < block.bodyTo) {
    const next = doc.sliceString(offset, Math.min(offset + 1, block.bodyTo));
    if (!next) break;
    offset += 1;
    if (next === "\n") lineCount += 1;
  }
  if (lineCount < line) return null;
  offset += column - 1;
  if (offset > block.bodyTo) return block.bodyTo;
  return offset;
}

/** Derive decoration set from the error-marks field + current blocks. */
function buildErrorDecorations(
  state: EditorState,
  blocks: DbFencedBlock[],
  marks: Map<string, DbErrorMark[]>,
): DecorationSet {
  if (marks.size === 0) return Decoration.none;
  const items: { from: number; to: number; deco: Decoration }[] = [];
  for (let i = 0; i < blocks.length; i++) {
    const block = blocks[i];
    const id = registry.blockIdOf(block, i);
    const blockMarks = marks.get(id);
    if (!blockMarks) continue;
    for (const mark of blockMarks) {
      const from = bodyLineColToOffset(
        state.doc,
        block,
        mark.line,
        mark.column,
      );
      if (from === null) continue;
      const length = Math.min(mark.length ?? 1, 32);
      const to = Math.min(from + Math.max(length, 1), block.bodyTo);
      if (to <= from) continue;
      items.push({
        from,
        to,
        deco: Decoration.mark({
          class: "cm-db-sql-error",
          attributes: mark.message ? { title: mark.message } : undefined,
        }),
      });
    }
  }
  items.sort((a, b) => a.from - b.from);
  const builder = new RangeSetBuilder<Decoration>();
  for (const { from, to, deco } of items) {
    builder.add(from, to, deco);
  }
  return builder.finish();
}

/**
 * Replace the error marks for a given block. Pass an empty array to clear.
 * Safe to call during render — the dispatch is scheduled.
 */
export function setDbBlockErrors(
  view: EditorView,
  blockId: string,
  marks: Omit<DbErrorMark, "blockId">[],
): void {
  const effects: StateEffect<unknown>[] = [clearDbErrorsEffect.of(blockId)];
  if (marks.length > 0) {
    effects.push(setDbErrorsEffect.of(marks.map((m) => ({ ...m, blockId }))));
  }
  view.dispatch({ effects });
}

// ───── Public extension factory ─────

export function createDbBlockExtension(): Extension {
  return createFencedBlockExtension({
    scanner,
    registry,
    buildDecorations: (state, blocks) =>
      buildFenceDecorations(state, blocks, {
        registry,
        classPrefix: "cm-db",
        ToolbarWidget: DbToolbarPortalWidget,
        ClosePanelWidget: DbClosePanelWidget,
        decorateBody: decorateDbBody,
      }),
    keymapBindings: [
      { key: "Mod-Enter", action: "onRun", requireHandler: false },
      { key: "Mod-.", action: "onCancel", requireHandler: false },
      { key: "Mod-Shift-e", action: "onExplain", requireHandler: true },
    ],
    extraExtensions: (getBlocks) => [
      dbErrorsField,
      // Error-mark decorations live in their own field so changes to the
      // marks map don't force a rebuild of the full fenced-block deco set.
      EditorView.decorations.compute([dbErrorsField], (state) =>
        buildErrorDecorations(state, getBlocks(), state.field(dbErrorsField)),
      ),
    ],
  });
}

// ───── Autocomplete source — {{ref}} (identical pattern to HTTP) ─────

export function createDbBlockCompletionSource(
  getFilePath: () => string | undefined,
): CompletionSource {
  return makeRefCompletionSource(findDbBlocks, getFilePath);
}

// ───── Exports for tests ─────

/**
 * Thin wrapper preserving the pre-A3 `__internal.buildDbDecorations` shape
 * so any consumer relying on it keeps working. The actual decoration logic
 * runs through the shared `buildFenceDecorations` skeleton.
 */
function buildDbDecorations(
  state: EditorState,
  blocks: DbFencedBlock[],
): DecorationSet {
  return buildFenceDecorations(state, blocks, {
    registry,
    classPrefix: "cm-db",
    ToolbarWidget: DbToolbarPortalWidget,
    ClosePanelWidget: DbClosePanelWidget,
    decorateBody: decorateDbBody,
  });
}

export const __internal = {
  DB_OPEN_RE,
  FENCE_CLOSE_RE,
  countDbBlocks: scanner.countBlocks,
  buildDbDecorations,
};
