/**
 * Generic CM6 fenced-block extension factory (A3 — collapses the parallel
 * skeleton between `cm-http-block.tsx` and `cm-db-block.tsx`: scanner,
 * decorations, keymap, StateField create/update, ref autocomplete).
 *
 * Genuine divergences kept type-specific (RULE 4 — not forced):
 *  - `parse(match)` — HTTP `(info)` vs DB `(lang, info)` capture groups.
 *  - `decorateBody` — HTTP renders method coloring, form-mode replace,
 *    per-line syntax classification, header-key marks; DB just paints
 *    line classes. Two callbacks, no shared "body decorator".
 *  - keymap bindings — HTTP {Mod-Enter requireHandler, Mod-., Mod-Shift-c};
 *    DB {Mod-Enter loose, Mod-., Mod-Shift-e requireHandler}.
 *  - extra extensions — DB hooks its error-mark StateField + decoration
 *    compute here; HTTP supplies none.
 */

import {
  EditorState,
  Prec,
  RangeSetBuilder,
  StateField,
  type Extension,
  type Text as CMText,
} from "@codemirror/state";
import {
  Decoration,
  EditorView,
  WidgetType,
  keymap,
  type DecorationSet,
  type KeyBinding,
} from "@codemirror/view";
import type {
  CompletionContext,
  CompletionResult,
  CompletionSource,
} from "@codemirror/autocomplete";

import {
  type DecoItem,
  type FencedBlockBase,
  type PortalEntryOf,
  type WidgetPortalRegistry,
} from "@/lib/codemirror/widget-portal-registry";
import { createReferenceCompletionSource } from "@/lib/blocks/cm-autocomplete";
import { collectBlocksAboveCM } from "@/lib/blocks/document";
import { useEnvironmentStore } from "@/stores/environment";

// Shared close-fence shape (`\`\`\`+\s*$`) — both block types use this.
export const FENCE_CLOSE_RE = /^```+\s*$/;

// ───── Scanner factory ─────

interface ScannerParse<Extras> {
  /** Extract type-specific fields (info/lang/metadata) from the open match. */
  (match: RegExpMatchArray): Extras;
}

export interface FencedScanner<Block extends FencedBlockBase> {
  findBlocks(doc: CMText): Block[];
  countBlocks(doc: CMText): number;
}

export function makeFencedScanner<
  Block extends FencedBlockBase,
  Extras extends Partial<Block>,
>({
  openRe,
  parse,
}: {
  openRe: RegExp;
  parse: ScannerParse<Extras>;
}): FencedScanner<Block> {
  function findBlocks(doc: CMText): Block[] {
    const blocks: Block[] = [];
    let inBlock = false;
    let openFrom = 0;
    let openTo = 0;
    let openMatch: RegExpMatchArray | null = null;
    let bodyStart = 0;
    const bodyLines: string[] = [];

    for (let i = 1; i <= doc.lines; i++) {
      const line = doc.line(i);
      const text = line.text;

      if (!inBlock) {
        const match = text.match(openRe);
        if (match) {
          inBlock = true;
          openFrom = line.from;
          openTo = line.to;
          openMatch = match;
          bodyStart = line.to + 1;
          bodyLines.length = 0;
        }
      } else {
        if (FENCE_CLOSE_RE.test(text)) {
          // Type-specific fields (info/lang/metadata) come from `parse`;
          // the positional skeleton below is `FencedBlockBase` minus
          // `info`, which the parse extras always supply.
          const extras = parse(openMatch as RegExpMatchArray);
          const block = {
            from: openFrom,
            to: line.to,
            openLineFrom: openFrom,
            openLineTo: openTo,
            bodyFrom: bodyStart,
            bodyTo: line.from === bodyStart ? bodyStart : line.from - 1,
            closeLineFrom: line.from,
            closeLineTo: line.to,
            body: bodyLines.join("\n"),
            ...extras,
          } as unknown as Block;
          blocks.push(block);
          inBlock = false;
          openMatch = null;
        } else {
          bodyLines.push(text);
        }
      }
    }
    return blocks;
  }

  function countBlocks(doc: CMText): number {
    let count = 0;
    for (let i = 1; i <= doc.lines; i++) {
      if (openRe.test(doc.line(i).text)) count++;
    }
    return count;
  }

  return { findBlocks, countBlocks };
}

// ───── Cursor / block lookup helpers ─────

export function cursorInsideBlock<Block extends FencedBlockBase>(
  state: EditorState,
  block: Block,
): boolean {
  const pos = state.selection.main.head;
  return pos >= block.from && pos <= block.to;
}

export function blockAtCursor<
  Slot extends string,
  Actions extends object,
  Block extends FencedBlockBase & { metadata: unknown },
>(
  view: EditorView,
  blocks: Block[],
  registry: WidgetPortalRegistry<Slot, Actions, Block>,
): { entry: PortalEntryOf<Slot, Actions, Block>; block: Block } | null {
  const pos = view.state.selection.main.head;
  const containers = registry.getContainers();
  for (let i = 0; i < blocks.length; i++) {
    const block = blocks[i];
    if (pos >= block.from && pos <= block.to) {
      const entry = containers.get(registry.blockIdOf(block, i));
      if (entry) return { entry, block };
      return null;
    }
  }
  return null;
}

// ───── Shared fence-decoration skeleton ─────

/**
 * Shape callbacks use to push items into the decoration set.
 * `order` resolves ties when two decos start at the same offset.
 */
export type PushItem = (item: DecoItem) => void;

export interface FenceSkeletonParams<
  Slot extends string,
  Actions extends object,
  Block extends FencedBlockBase & { metadata: unknown },
> {
  registry: WidgetPortalRegistry<Slot, Actions, Block>;
  classPrefix: string; // "cm-http" | "cm-db"
  ToolbarWidget: new (blockId: string, block: Block) => WidgetType;
  ClosePanelWidget: new (blockId: string, block: Block) => WidgetType;
  /** Push body-line / body-replace decorations (type-specific). */
  decorateBody: (
    state: EditorState,
    block: Block,
    blockId: string,
    editing: boolean,
    push: PushItem,
  ) => void;
}

/**
 * Build the editing/reading fence skeleton common to every fenced block
 * type, delegating body rendering to the caller. The body callback owns
 * the per-line classes, replacement widgets, and any per-line marks.
 */
export function buildFenceDecorations<
  Slot extends string,
  Actions extends object,
  Block extends FencedBlockBase & { metadata: unknown },
>(
  state: EditorState,
  blocks: Block[],
  params: FenceSkeletonParams<Slot, Actions, Block>,
): DecorationSet {
  const items: DecoItem[] = [];
  const push: PushItem = (it) => items.push(it);
  const {
    registry,
    classPrefix,
    ToolbarWidget,
    ClosePanelWidget,
    decorateBody,
  } = params;

  for (let i = 0; i < blocks.length; i++) {
    const block = blocks[i];
    const blockId = registry.blockIdOf(block, i);
    const editing = cursorInsideBlock(state, block);

    if (editing) {
      // Editing: show raw fence text with subtle styling.
      items.push({
        from: block.openLineFrom,
        to: block.openLineFrom,
        deco: Decoration.line({
          class: `${classPrefix}-fence-line ${classPrefix}-fence-line-open`,
        }),
        order: 0,
      });
      items.push({
        from: block.closeLineFrom,
        to: block.closeLineFrom,
        deco: Decoration.line({
          class: `${classPrefix}-fence-line ${classPrefix}-fence-line-close`,
        }),
        order: 0,
      });
      // Result panel still visible while editing — single block widget
      // after the close fence (side: 1) so cursor navigation past the
      // block stays consistent with the reading-mode replacement.
      items.push({
        from: block.closeLineTo,
        to: block.closeLineTo,
        deco: Decoration.widget({
          widget: new ClosePanelWidget(blockId, block),
          block: true,
          side: 1,
        }),
        order: 3,
      });
    } else {
      // Reading: hide fences. Open fence becomes the toolbar/header.
      items.push({
        from: block.openLineFrom,
        to: block.openLineTo,
        deco: Decoration.replace({
          widget: new ToolbarWidget(blockId, block),
          block: true,
        }),
        order: 0,
      });
      // Close fence + result tabs + status bar as a single block widget.
      items.push({
        from: block.closeLineFrom,
        to: block.closeLineTo,
        deco: Decoration.replace({
          widget: new ClosePanelWidget(blockId, block),
          block: true,
        }),
        order: 1,
      });
    }

    decorateBody(state, block, blockId, editing, push);
  }

  items.sort((a, b) => a.from - b.from || a.order - b.order);

  const builder = new RangeSetBuilder<Decoration>();
  for (const { from, to, deco } of items) {
    builder.add(from, to, deco);
  }
  return builder.finish();
}

// ───── Keymap ─────

export interface KeymapBindingSpec<Actions extends object> {
  key: string;
  action: keyof Actions;
  /**
   * When true, the binding returns `false` (lets the key fall through) if
   * the resolved action handler is missing. When false, it returns true
   * unconditionally when the cursor is inside a block, calling the handler
   * optionally — DB's Mod-Enter / Mod-. behavior.
   */
  requireHandler: boolean;
}

export function makeFencedKeymap<
  Slot extends string,
  Actions extends object,
  Block extends FencedBlockBase & { metadata: unknown },
>(
  getBlocks: () => Block[],
  registry: WidgetPortalRegistry<Slot, Actions, Block>,
  bindings: KeymapBindingSpec<Actions>[],
): KeyBinding[] {
  return bindings.map((spec) => ({
    key: spec.key,
    run: (view) => {
      const found = blockAtCursor(view, getBlocks(), registry);
      if (!found) return false;
      const handler = found.entry.actions[spec.action] as unknown as
        | (() => void)
        | undefined;
      if (spec.requireHandler && !handler) return false;
      handler?.();
      return true;
    },
  }));
}

// ───── Extension factory ─────

export interface CreateFencedBlockExtensionParams<
  Slot extends string,
  Actions extends object,
  Block extends FencedBlockBase & { metadata: unknown },
> {
  scanner: FencedScanner<Block>;
  registry: WidgetPortalRegistry<Slot, Actions, Block>;
  buildDecorations: (state: EditorState, blocks: Block[]) => DecorationSet;
  keymapBindings: KeymapBindingSpec<Actions>[];
  /**
   * Additional extensions (e.g. DB's error-mark StateField + compute).
   * Receives a getter for the live `cachedBlocks` so an external compute
   * can read fresh positions without owning the cache.
   */
  extraExtensions?: (getBlocks: () => Block[]) => Extension[];
}

export function createFencedBlockExtension<
  Slot extends string,
  Actions extends object,
  Block extends FencedBlockBase & { metadata: unknown },
>(params: CreateFencedBlockExtensionParams<Slot, Actions, Block>): Extension {
  const {
    scanner,
    registry,
    buildDecorations,
    keymapBindings,
    extraExtensions,
  } = params;
  let cachedBlocks: Block[] = [];
  let lastBlockCount = 0;

  const field = StateField.define<DecorationSet>({
    create(state) {
      cachedBlocks = scanner.findBlocks(state.doc);
      lastBlockCount = cachedBlocks.length;
      registry.syncBlocks(cachedBlocks);
      return buildDecorations(state, cachedBlocks);
    },
    update(decos, tr) {
      if (tr.docChanged) {
        const newCount = scanner.countBlocks(tr.state.doc);
        if (newCount !== lastBlockCount) {
          lastBlockCount = newCount;
        }
        cachedBlocks = scanner.findBlocks(tr.state.doc);
        // Keep the portal registry's `entry.block` in sync so the React
        // panel's metadata/body reflect the doc edit immediately.
        registry.syncBlocks(cachedBlocks);
        return buildDecorations(tr.state, cachedBlocks);
      }
      if (tr.selection) {
        // Selection moved — only rebuild if this crosses an edit-mode
        // boundary of some block.
        const oldPos = tr.startState.selection.main.head;
        const newPos = tr.state.selection.main.head;
        const crossed = cachedBlocks.some((b) => {
          const oldInside = oldPos >= b.from && oldPos <= b.to;
          const newInside = newPos >= b.from && newPos <= b.to;
          return oldInside !== newInside;
        });
        if (crossed) {
          return buildDecorations(tr.state, cachedBlocks);
        }
      }
      return decos;
    },
    provide: (f) => EditorView.decorations.from(f),
  });

  // Prec.high ensures ⌘↵ / ⌘. win over @codemirror/commands' defaultKeymap,
  // which binds `Mod-Enter` → `insertBlankLine`. Without this, the default
  // binding consumes the event before our handler runs.
  const km = Prec.high(
    keymap.of(makeFencedKeymap(() => cachedBlocks, registry, keymapBindings)),
  );

  const extras = extraExtensions ? extraExtensions(() => cachedBlocks) : [];
  return [field, km, ...extras];
}

// ───── Ref autocomplete (identical in both block types) ─────

/**
 * Build a `{{ref}}` completion source bound to a specific block-finder.
 * Offers block aliases (from blocks above the cursor) and non-secret env
 * variable keys. Returns null outside any block's body.
 */
export function makeRefCompletionSource<Block extends FencedBlockBase>(
  findBlocks: (doc: CMText) => Block[],
  getFilePath: () => string | undefined,
): CompletionSource {
  return async (ctx: CompletionContext): Promise<CompletionResult | null> => {
    const pos = ctx.pos;
    const blocks = findBlocks(ctx.state.doc);
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
