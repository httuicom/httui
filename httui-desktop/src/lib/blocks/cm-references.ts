import {
  ViewPlugin,
  Decoration,
  type DecorationSet,
  EditorView,
  type ViewUpdate,
  MatchDecorator,
  hoverTooltip,
  type Tooltip,
} from "@codemirror/view";
import type { Extension } from "@codemirror/state";
import {
  parseReferences,
  resolveReference,
  type BlockContext,
} from "./references";
import { collectBlocksAboveCM } from "./document";
import { findFencedBlocks } from "@/lib/codemirror/cm-block-widgets";
import { findDbBlocks } from "@/lib/codemirror/cm-db-block";

const REF_REGEX = /\{\{[^}]+\}\}/g;

const refMark = Decoration.mark({ class: "cm-reference-highlight" });

const decorator = new MatchDecorator({
  regexp: REF_REGEX,
  decoration: () => refMark,
});

const referenceHighlightPlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = decorator.createDeco(view);
    }
    update(update: ViewUpdate) {
      this.decorations = decorator.updateDeco(update, this.decorations);
    }
  },
  { decorations: (v) => v.decorations },
);

const referenceHighlightTheme = EditorView.baseTheme({
  ".cm-reference-highlight": {
    backgroundColor: "rgba(139, 92, 246, 0.15)",
    borderRadius: "3px",
    color: "rgb(139, 92, 246)",
  },
  ".cm-ref-tooltip": {
    fontFamily: "var(--chakra-fonts-mono)",
    fontSize: "11px",
    padding: "4px 8px",
    borderRadius: "4px",
    maxWidth: "400px",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "pre-wrap",
    wordBreak: "break-all",
    maxHeight: "200px",
    overflowY: "auto",
    // Never capture mouse events — the tooltip floats above the text and
    // would otherwise intercept double-click / select / etc. on the span
    // it's anchored to (and make `{{…}}` feel "dead" to interact with).
    pointerEvents: "none",
  },
  ".cm-ref-tooltip-value": {
    color: "rgb(139, 92, 246)",
  },
  ".cm-ref-tooltip-error": {
    color: "rgb(239, 68, 68)",
  },
});

/**
 * CodeMirror extension that highlights {{...}} reference patterns.
 */
export const referenceHighlight = [
  referenceHighlightPlugin,
  referenceHighlightTheme,
];

function truncateValue(val: string, maxLen = 200): string {
  return val.length > maxLen ? val.slice(0, maxLen) + "..." : val;
}

/**
 * Create a hover tooltip extension that resolves {{...}} references
 * and shows the value or error on hover.
 *
 * @param getBlocks - getter returning current block contexts
 * @param getCurrentPos - getter returning current block position in doc
 * @param getEnvVars - getter returning active environment variables
 */
export function createReferenceTooltip(
  getBlocks: () => BlockContext[],
  getCurrentPos: () => number,
  getEnvVars?: () => Record<string, string>,
): Extension {
  return hoverTooltip((view, pos): Tooltip | null => {
    const { state } = view;
    const line = state.doc.lineAt(pos);
    const lineText = line.text;

    // Find {{...}} pattern at hover position
    const regex = /\{\{([^}]+)\}\}/g;
    let match: RegExpExecArray | null;
    while ((match = regex.exec(lineText)) !== null) {
      const from = line.from + match.index;
      const to = from + match[0].length;

      if (pos >= from && pos <= to) {
        const refs = parseReferences(match[0]);
        if (refs.length === 0) return null;
        const ref = refs[0];

        const blocks = getBlocks();
        const currentPos = getCurrentPos();
        const envVars = getEnvVars?.();

        let resolvedText: string;
        let isError = false;

        // Same priority as resolveAllReferences: block ref > env var
        const matchingBlock = blocks.find(
          (b) => b.alias === ref.alias && b.pos < currentPos,
        );
        if (matchingBlock) {
          try {
            resolvedText = resolveReference(ref, blocks, currentPos);
          } catch (err) {
            resolvedText = err instanceof Error ? err.message : String(err);
            isError = true;
          }
        } else if (ref.path.length === 0 && envVars && ref.alias in envVars) {
          resolvedText = envVars[ref.alias];
        } else {
          try {
            resolvedText = resolveReference(ref, blocks, currentPos);
          } catch (err) {
            resolvedText = err instanceof Error ? err.message : String(err);
            isError = true;
          }
        }

        return {
          pos: from,
          end: to,
          above: true,
          create() {
            const dom = document.createElement("div");
            dom.className = `cm-ref-tooltip ${isError ? "cm-ref-tooltip-error" : "cm-ref-tooltip-value"}`;
            dom.textContent = isError
              ? `⚠ ${resolvedText}`
              : truncateValue(resolvedText);
            return { dom };
          },
        };
      }
    }
    return null;
  });
}

/**
 * Hover tooltip for `{{ref}}` inside the main markdown editor — primarily
 * for DB block bodies (SQL lives directly in the CM6 document, not in a
 * sub-editor). Differs from `createReferenceTooltip` in two ways:
 *   1. The enclosing block is discovered dynamically from the hover
 *      position (DB block or http/e2e fence).
 *   2. Block contexts are fetched async because cache lookup hits SQLite.
 *      `hoverTooltip` itself is synchronous, so we return a placeholder
 *      DOM and mutate it once the async resolve completes.
 */
export function createMarkdownReferenceTooltip(
  getFilePath: () => string | undefined,
  getEnvVars?: () => Promise<Record<string, string>> | Record<string, string>,
): Extension {
  return hoverTooltip((view, pos): Tooltip | null => {
    const { state } = view;
    const line = state.doc.lineAt(pos);
    const lineText = line.text;

    // Find a `{{…}}` whose range contains `pos`. CM6 can sometimes hand us
    // a position a couple of chars off the span edge, so we also accept a
    // small tolerance window around each match.
    const regex = /\{\{([^}]+)\}\}/g;
    let chosen: { raw: string; from: number; to: number } | null = null;
    let best: { raw: string; from: number; to: number; dist: number } | null =
      null;
    for (let m: RegExpExecArray | null; (m = regex.exec(lineText)) !== null; ) {
      const from = line.from + m.index;
      const to = line.from + m.index + m[0].length;
      if (pos >= from && pos <= to) {
        chosen = { raw: m[0], from, to };
        break;
      }
      const dist = pos < from ? from - pos : pos - to;
      if (dist <= 4 && (!best || dist < best.dist)) {
        best = { raw: m[0], from, to, dist };
      }
    }
    if (!chosen && best)
      chosen = { raw: best.raw, from: best.from, to: best.to };
    if (!chosen) return null;

    const refs = parseReferences(chosen.raw);
    if (refs.length === 0) return null;
    const ref = refs[0];
    const refFrom = chosen.from;
    const refTo = chosen.to;

    // Find the enclosing executable block so we can resolve refs relative
    // to "blocks above this block". DB blocks expose a body range; http /
    // e2e fences cover the whole block. Either is fine — we just need
    // `.from` to drive the "above" filter.
    let blockFrom: number | null = null;
    for (const b of findDbBlocks(state.doc)) {
      if (pos >= b.bodyFrom && pos <= b.bodyTo) {
        blockFrom = b.from;
        break;
      }
    }
    if (blockFrom === null) {
      for (const b of findFencedBlocks(state.doc)) {
        if (pos >= b.from && pos <= b.to) {
          blockFrom = b.from;
          break;
        }
      }
    }
    if (blockFrom === null) return null;

    const filePath = getFilePath();
    if (!filePath) return null;

    return {
      pos: refFrom,
      end: refTo,
      above: true,
      create() {
        const dom = document.createElement("div");
        dom.className = "cm-ref-tooltip cm-ref-tooltip-value";
        dom.textContent = "Resolving…";

        (async () => {
          const blocks = await collectBlocksAboveCM(
            view.state.doc,
            blockFrom,
            filePath,
          );
          const envVars = (await getEnvVars?.()) ?? {};

          let text: string;
          let isError = false;
          try {
            const matchingBlock = blocks.find(
              (b) => b.alias === ref.alias && b.pos < blockFrom,
            );
            if (matchingBlock) {
              text = resolveReference(ref, blocks, blockFrom);
            } else if (ref.path.length === 0 && ref.alias in envVars) {
              text = envVars[ref.alias];
            } else {
              text = resolveReference(ref, blocks, blockFrom);
            }
          } catch (err) {
            text = err instanceof Error ? err.message : String(err);
            isError = true;
          }

          dom.className = `cm-ref-tooltip ${isError ? "cm-ref-tooltip-error" : "cm-ref-tooltip-value"}`;
          dom.textContent = isError ? `⚠ ${text}` : truncateValue(text);
        })().catch((err) => {
          dom.className = "cm-ref-tooltip cm-ref-tooltip-error";
          dom.textContent = `⚠ ${err instanceof Error ? err.message : String(err)}`;
        });

        return { dom };
      },
    };
  });
}
