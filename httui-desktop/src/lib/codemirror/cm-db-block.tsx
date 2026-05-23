/**
 * CodeMirror extension for DB block rendering.
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

export interface DbFencedBlock {
  from: number;
  to: number;
  lang: string;
  metadata: DbBlockMetadata;
}

const DB_OPEN_RE = /^```(db(?:-[\w:-]+)?)(.*)$/;
const FENCE_CLOSE_RE = /^```+\s*$/;

export function findDbBlocks(doc: CMText): DbFencedBlock[] {
  const blocks: DbFencedBlock[] = [];
  let inBlock = false;
  let openFrom = 0;
  let openTo = 0;
  let lang = "";
  let info = "";
  let bodyStart = 0;
  const bodyLines: string[] = [];

  for (let i = 1; i <= doc.lines; i++) {
    const line = doc.line(i);
    const text = line.text;

    if (!inBlock) {
      const match = text.match(DB_OPEN_RE);
      if (match) {
        inBlock = true;
        openFrom = line.from;
        openTo = line.to;
        lang = match[1];
        info = match[2].trim();
        bodyStart = line.to + 1;
        bodyLines.length = 0;
      }
    } else {
      if (FENCE_CLOSE_RE.test(text)) {
        const metadata = parseDbFenceInfo(`${lang} ${info}`.trim()) ?? {
          dialect: "generic",
        };
        blocks.push({
          from: openFrom,
          to: line.to,
          lang,
          info,
          metadata,
          openLineFrom: openFrom,
          openLineTo: openTo,
          bodyFrom: bodyStart,
          bodyTo: line.from === bodyStart ? bodyStart : line.from - 1,
          closeLineFrom: line.from,
          closeLineTo: line.to,
          body: bodyLines.join("\n"),
        });
        inBlock = false;
      } else {
        bodyLines.push(text);
      }
    }
  }

  return blocks;
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

// Body-only changes debounce notify so body-dependent effects catch up
// without re-rendering on every keystroke. 250ms matches idle typing.
let bodyNotifyTimer: ReturnType<typeof setTimeout> | null = null;
function scheduleBodyNotify() {
  if (bodyNotifyTimer !== null) clearTimeout(bodyNotifyTimer);
  bodyNotifyTimer = setTimeout(() => {
    bodyNotifyTimer = null;
    notify();
  }, 250);
}

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
export function setDbBlockActions(
  blockId: string,
  actions: DbPortalActions,
): void {
  const entry = entries.get(blockId);
  if (!entry) return;
  entry.actions = { ...entry.actions, ...actions };
}

/**
 * Slot (un)register creates a NEW entry object — `DbFencedPanel` is wrapped
 * in React.memo and would skip re-render if the `entry` prop kept the same
 * reference across slot changes, leaving the toolbar unmounted after the
 * cursor leaves the block.
 */
function registerSlot(
  blockId: string,
  block: DbFencedBlock,
  slot: DbWidgetSlot,
  element: HTMLElement,
) {
  const prev = entries.get(blockId);
  const next: DbPortalEntry = prev
    ? { ...prev, block, [slot]: element }
    : { blockId, block, actions: {}, [slot]: element };
  entries.set(blockId, next);
  notify();
}

function unregisterSlot(blockId: string, slot: DbWidgetSlot) {
  const prev = entries.get(blockId);
  if (!prev) return;
  const next: DbPortalEntry = { ...prev, [slot]: undefined };
  if (!next.toolbar && !next.result && !next.statusbar) {
    entries.delete(blockId);
  } else {
    entries.set(blockId, next);
  }
  notify();
}

/**
 * Build a block id. Index-based so metadata edits (especially the alias)
 * don't swap the id under us — the React panel state must stay bound to
 * the same block while the user is editing it. Insert-above reordering
 * does migrate state, which is a lesser evil than losing drawer focus
 * on every keystroke. A future stable-id-in-info-string proposal can
 * fix the reorder case without reintroducing the edit-swap.
 */
function blockIdOf(_block: DbFencedBlock, index: number): string {
  return `db_idx_${index}`;
}

/**
 * Keep each registry entry in sync with the latest block scan. Only swaps
 * `entry.block` (and calls `notify()`) when metadata or body changed —
 * things the React panel actually renders. Position-only shifts mutate the
 * existing block object in place so React.memo skips re-rendering.
 */
function syncRegistryBlocks(blocks: DbFencedBlock[]): void {
  let meaningfulChange = false;
  for (let i = 0; i < blocks.length; i++) {
    const id = blockIdOf(blocks[i], i);
    const entry = entries.get(id);
    if (!entry) continue;
    const prev = entry.block;
    const fresh = blocks[i];
    if (prev === fresh) continue;

    const prevMeta = prev.metadata;
    const nextMeta = fresh.metadata;
    const metaChanged =
      prevMeta.dialect !== nextMeta.dialect ||
      prevMeta.alias !== nextMeta.alias ||
      prevMeta.connection !== nextMeta.connection ||
      prevMeta.limit !== nextMeta.limit ||
      prevMeta.timeoutMs !== nextMeta.timeoutMs ||
      prevMeta.displayMode !== nextMeta.displayMode;
    const bodyChanged = prev.body !== fresh.body;

    if (metaChanged) {
      entry.block = fresh;
      meaningfulChange = true;
    } else if (bodyChanged) {
      // Debounce body-only changes so React doesn't re-render the whole
      // panel on every keystroke in the query.
      entry.block = fresh;
      scheduleBodyNotify();
    } else {
      // Same meta + same body — pure position shift. Mutate in place so
      // React.memo skips the render entirely.
      prev.from = fresh.from;
      prev.to = fresh.to;
      prev.bodyFrom = fresh.bodyFrom;
      prev.bodyTo = fresh.bodyTo;
      prev.openLineFrom = fresh.openLineFrom;
      prev.openLineTo = fresh.openLineTo;
      prev.closeLineFrom = fresh.closeLineFrom;
      prev.closeLineTo = fresh.closeLineTo;
      prev.lang = fresh.lang;
      prev.info = fresh.info;
    }
  }
  if (meaningfulChange) notify();
}

class DbToolbarPortalWidget extends WidgetType {
  constructor(
    readonly blockId: string,
    readonly block: DbFencedBlock,
  ) {
    super();
  }

  toDOM(view: EditorView): HTMLElement {
    const div = document.createElement("div");
    div.className = "cm-db-toolbar-portal";
    div.contentEditable = "false";
    registerSlot(this.blockId, this.block, "toolbar", div);
    observeWidgetHeight(div, this.blockId, "toolbar", view);
    return div;
  }

  updateDOM(dom: HTMLElement): boolean {
    registerSlot(this.blockId, this.block, "toolbar", dom);
    return true;
  }

  destroy(dom: HTMLElement): void {
    disconnectWidgetObserver(dom, this.blockId, "toolbar");
    unregisterSlot(this.blockId, "toolbar");
  }

  eq(other: DbToolbarPortalWidget): boolean {
    return this.blockId === other.blockId;
  }

  get estimatedHeight(): number {
    const cached = widgetHeightCache.get(cacheKey(this.blockId, "toolbar"));
    return cached ?? 44;
  }

  ignoreEvent(): boolean {
    // Toolbar handles its own clicks via React — don't let them bubble
    // into CM6 as cursor-positioning events.
    return true;
  }
}

/**
 * Module-level DOM-height cache per `${blockId}:${slot}`. `estimatedHeight`
 * reads from here so CM6's `moveVertically` math stays consistent with the
 * widget's actual rendered size — critical when a result panel expands
 * from ~80px to ~400px between arrow-key presses, which otherwise confuses
 * cursor navigation and teleports the caret.
 */
const widgetHeightCache = new Map<string, number>();

function cacheKey(blockId: string, slot: DbWidgetSlot): string {
  return `${blockId}:${slot}`;
}

function observeWidgetHeight(
  dom: HTMLElement,
  blockId: string,
  slot: DbWidgetSlot,
  view: EditorView,
): void {
  if (typeof ResizeObserver === "undefined") return;
  // Seed with `offsetHeight` (border-box) so the cached value matches what
  // CM6 measures via `getBoundingClientRect`. `ResizeObserver.contentRect`
  // is content-box and drops padding/border, which made clicks below the
  // block land one line too low. Skip when 0 — that happens before CM6
  // attaches the widget; caching 0 would poison `estimatedHeight`.
  const seed = dom.offsetHeight;
  if (seed > 0) widgetHeightCache.set(cacheKey(blockId, slot), seed);
  const ro = new ResizeObserver(() => {
    const prev = widgetHeightCache.get(cacheKey(blockId, slot));
    const next = dom.offsetHeight;
    if (next > 0 && prev !== next) {
      widgetHeightCache.set(cacheKey(blockId, slot), next);
      // Re-measure so CM6's internal block info stays accurate; without
      // this, `moveVertically` uses the stale estimate and the caret drifts.
      view.requestMeasure();
    }
  });
  ro.observe(dom);
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (dom as any).__cmWidgetResizeObserver = ro;
}

function disconnectWidgetObserver(
  dom: HTMLElement | undefined,
  blockId: string,
  slot: DbWidgetSlot,
): void {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const ro = (dom as any)?.__cmWidgetResizeObserver as
    | ResizeObserver
    | undefined;
  ro?.disconnect();
  widgetHeightCache.delete(cacheKey(blockId, slot));
}

/**
 * Single block widget replacing the close-fence line with the full post-body
 * UI (fence spacer + result tabs + status bar). Keeping them in one widget is
 * necessary because CM6's `moveVertically` gets confused by multiple adjacent
 * `block: true` decorations — the probe Y overshoots and teleports the caret
 * to line 1.
 */
class DbClosePanelWidget extends WidgetType {
  constructor(
    readonly blockId: string,
    readonly block: DbFencedBlock,
  ) {
    super();
  }

  toDOM(view: EditorView): HTMLElement {
    const wrap = document.createElement("div");
    wrap.className = "cm-db-close-panel";
    wrap.contentEditable = "false";

    const spacer = document.createElement("div");
    spacer.className = "cm-db-fence-hidden";
    wrap.appendChild(spacer);

    const result = document.createElement("div");
    result.className = "cm-db-result-portal";
    registerSlot(this.blockId, this.block, "result", result);
    wrap.appendChild(result);

    const status = document.createElement("div");
    status.className = "cm-db-statusbar-portal";
    registerSlot(this.blockId, this.block, "statusbar", status);
    wrap.appendChild(status);

    observeWidgetHeight(wrap, this.blockId, "result", view);
    return wrap;
  }

  updateDOM(dom: HTMLElement): boolean {
    const result = dom.querySelector(".cm-db-result-portal");
    const status = dom.querySelector(".cm-db-statusbar-portal");
    if (result instanceof HTMLElement) {
      registerSlot(this.blockId, this.block, "result", result);
    }
    if (status instanceof HTMLElement) {
      registerSlot(this.blockId, this.block, "statusbar", status);
    }
    return true;
  }

  destroy(dom: HTMLElement): void {
    disconnectWidgetObserver(dom, this.blockId, "result");
    unregisterSlot(this.blockId, "result");
    unregisterSlot(this.blockId, "statusbar");
  }

  eq(other: DbClosePanelWidget): boolean {
    return this.blockId === other.blockId;
  }

  get estimatedHeight(): number {
    const cached = widgetHeightCache.get(cacheKey(this.blockId, "result"));
    return cached ?? 120;
  }

  ignoreEvent(): boolean {
    return true;
  }
}

function cursorInsideBlock(state: EditorState, block: DbFencedBlock): boolean {
  const pos = state.selection.main.head;
  return pos >= block.from && pos <= block.to;
}

function buildDbDecorations(
  state: EditorState,
  blocks: DbFencedBlock[],
): DecorationSet {
  type Item = {
    from: number;
    to: number;
    deco: Decoration;
    order: number;
  };
  const items: Item[] = [];

  for (let i = 0; i < blocks.length; i++) {
    const block = blocks[i];
    const blockId = blockIdOf(block, i);
    const editing = cursorInsideBlock(state, block);

    if (editing) {
      items.push({
        from: block.openLineFrom,
        to: block.openLineFrom,
        deco: Decoration.line({
          class: "cm-db-fence-line cm-db-fence-line-open",
        }),
        order: 0,
      });
      items.push({
        from: block.closeLineFrom,
        to: block.closeLineFrom,
        deco: Decoration.line({
          class: "cm-db-fence-line cm-db-fence-line-close",
        }),
        order: 0,
      });
      // Result panel visible while editing — single block widget after the
      // close fence (side: 1). Same widget class as reading-mode so content
      // survives mode toggles without re-mounting.
      items.push({
        from: block.closeLineTo,
        to: block.closeLineTo,
        deco: Decoration.widget({
          widget: new DbClosePanelWidget(blockId, block),
          block: true,
          side: 1,
        }),
        order: 3,
      });
    } else {
      items.push({
        from: block.openLineFrom,
        to: block.openLineTo,
        deco: Decoration.replace({
          widget: new DbToolbarPortalWidget(blockId, block),
          block: true,
        }),
        order: 0,
      });
      items.push({
        from: block.closeLineFrom,
        to: block.closeLineTo,
        deco: Decoration.replace({
          widget: new DbClosePanelWidget(blockId, block),
          block: true,
        }),
        order: 1,
      });
    }

    if (block.body.length > 0) {
      const firstBodyLine = state.doc.lineAt(block.bodyFrom).number;
      const lastBodyLine = state.doc.lineAt(block.bodyTo).number;
      for (let n = firstBodyLine; n <= lastBodyLine; n++) {
        const line = state.doc.line(n);
        const classes = ["cm-db-body-line"];
        if (editing) classes.push("cm-db-body-editing");
        if (n === firstBodyLine) classes.push("cm-db-body-line-first");
        if (n === lastBodyLine) classes.push("cm-db-body-line-last");
        items.push({
          from: line.from,
          to: line.from,
          deco: Decoration.line({ class: classes.join(" ") }),
          order: 0,
        });
      }
    }
  }
}

function countDbBlocks(doc: CMText): number {
  let count = 0;
  for (let i = 1; i <= doc.lines; i++) {
    if (DB_OPEN_RE.test(doc.line(i).text)) count++;
  }
  return count;
}

function blockAtCursor(
  view: EditorView,
  blocks: DbFencedBlock[],
): { entry: DbPortalEntry; block: DbFencedBlock } | null {
  const pos = view.state.selection.main.head;
  for (let i = 0; i < blocks.length; i++) {
    const block = blocks[i];
    if (pos >= block.from && pos <= block.to) {
      const entry = entries.get(blockIdOf(block, i));
      if (entry) return { entry, block };
      return null;
    }
  }
  return null;
}

function makeKeymap(getBlocks: () => DbFencedBlock[]): KeyBinding[] {
  return [
    {
      key: "Mod-Enter",
      run: (view) => {
        const found = blockAtCursor(view, getBlocks());
        if (!found) return false;
        found.entry.actions.onRun?.();
        return true;
      },
    },
    {
      key: "Mod-.",
      run: (view) => {
        const found = blockAtCursor(view, getBlocks());
        if (!found) return false;
        found.entry.actions.onCancel?.();
        return true;
      },
    },
    {
      key: "Mod-Shift-e",
      run: (view) => {
        const found = blockAtCursor(view, getBlocks());
        if (!found || !found.entry.actions.onExplain) return false;
        found.entry.actions.onExplain();
        return true;
      },
    },
  ];
}

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

/** Map of `blockId` → current error marks. Rebuilt only when an effect fires. */
const dbErrorsField = StateField.define<Map<string, DbErrorMark[]>>({
  create: () => new Map(),
  update(value, tr) {
    let next = value;
    for (const effect of tr.effects) {
      if (effect.is(setDbErrorsEffect)) {
        next = new Map(next);
        const marks = effect.value;
        if (marks.length === 0) continue;
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

export function createDbBlockExtension(): Extension {
  let cachedBlocks: DbFencedBlock[] = [];
  let lastBlockCount = 0;

  const field = StateField.define<DecorationSet>({
    create(state) {
      cachedBlocks = findDbBlocks(state.doc);
      lastBlockCount = cachedBlocks.length;
      syncRegistryBlocks(cachedBlocks);
      return buildDbDecorations(state, cachedBlocks);
    },
    update(decos, tr) {
      if (tr.docChanged) {
        const newCount = countDbBlocks(tr.state.doc);
        if (newCount !== lastBlockCount) {
          lastBlockCount = newCount;
        }
        cachedBlocks = findDbBlocks(tr.state.doc);
        syncRegistryBlocks(cachedBlocks);
        return buildDbDecorations(tr.state, cachedBlocks);
      }
      if (tr.selection) {
        // Only rebuild when the cursor crosses an edit-mode boundary.
        const oldPos = tr.startState.selection.main.head;
        const newPos = tr.state.selection.main.head;
        const crossed = cachedBlocks.some((b) => {
          const oldInside = oldPos >= b.from && oldPos <= b.to;
          const newInside = newPos >= b.from && newPos <= b.to;
          return oldInside !== newInside;
        });
        if (crossed) {
          return buildDbDecorations(tr.state, cachedBlocks);
        }
      }
      return decos;
    },
    provide: (f) => EditorView.decorations.from(f),
  });

  // Prec.high ensures ⌘↵ / ⌘. win over defaultKeymap's `insertBlankLine`.
  const dbKeymap = Prec.high(keymap.of(makeKeymap(() => cachedBlocks)));

  // Error-mark decorations in their own field so mark changes don't force
  // a rebuild of the full fenced-block decorations.
  const errorDecos = EditorView.decorations.compute([dbErrorsField], (state) =>
    buildErrorDecorations(state, cachedBlocks, state.field(dbErrorsField)),
  );

  return [field, dbKeymap, dbErrorsField, errorDecos];
}

/**
 * CompletionSource for `{{ref}}` inside a db block body. Off outside db
 * blocks. Completes block aliases above the cursor, non-secret env-variable
 * keys, and path keys inside an existing `{{alias.…}}` reference.
 */
export function createDbBlockCompletionSource(
  getFilePath: () => string | undefined,
): CompletionSource {
  return async (ctx: CompletionContext): Promise<CompletionResult | null> => {
    const pos = ctx.pos;
    const blocks = findDbBlocks(ctx.state.doc);
    const inside = blocks.find((b) => pos >= b.bodyFrom && pos <= b.bodyTo);
    if (!inside) return null;

    const filePath = getFilePath();
    if (!filePath) return null;

    const contexts = await collectBlocksAboveCM(
      ctx.state.doc,
      inside.from,
      filePath,
    );
    const envVars = await useEnvironmentStore.getState().getActiveVariables();
    const envKeys = Object.keys(envVars);

    const source = createReferenceCompletionSource(
      () => contexts,
      () => envKeys,
    );
    return source(ctx);
  };
}

/** Lazy connections cache — refreshed on miss so new connections appear without a reload. */
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

/**
 * Returns a CompletionSource that offers table + column completions inside
 * a db block body, driven by the shared SchemaCache store.
 *
 * Behavior:
 *  - Off outside a db block body.
 *  - Off inside an active `{{ref}}` expression (ref autocomplete owns that).
 *  - After `FROM`/`JOIN`/`UPDATE`/`INTO` → tables.
 *  - After `table.` → columns of that table.
 *  - Elsewhere → tables + columns of tables already referenced in the body.
 *
 * Schema is pulled synchronously from the cache; on miss the store kicks
 * off an introspection in the background so the next keystroke will have
 * completions available.
 */
export function createDbSchemaCompletionSource(): CompletionSource {
  return async (ctx: CompletionContext): Promise<CompletionResult | null> => {
    const pos = ctx.pos;
    const blocks = findDbBlocks(ctx.state.doc);
    const block = blocks.find((b) => pos >= b.bodyFrom && pos <= b.bodyTo);
    if (!block) return null;

    // Inside a `{{ref}}` expression — let the ref source handle it.
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

    // Missing/orphan connections degrade the result set but we still offer
    // SQL keywords so Ctrl-Space never produces an empty popup.
    const identifier = block.metadata.connection;
    const connection = identifier
      ? resolveConnectionIdentifier(await ensureConnections(), identifier)
      : null;

    const dialect = effectiveDialect(
      block.metadata.dialect,
      connection?.driver,
    );

    const store = useSchemaCacheStore.getState();
    const schema = connection ? store.get(connection.id) : null;
    const schemaLoaded = !!schema;
    if (connection && !schema) {
      void store.ensureLoaded(connection.id);
    }

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
      // Unknown prefix — fall through to keyword completion.
    }

    const keywordOptions = keywordsFor(dialect);

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

export const __internal = {
  DB_OPEN_RE,
  FENCE_CLOSE_RE,
  countDbBlocks: scanner.countBlocks,
  buildDbDecorations,
};
