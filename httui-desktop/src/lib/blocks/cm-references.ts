// First-paint highlight for `{{ref}}` spans, driven by the
// @httui/lezer-refs grammar (the lexical mirror of the canonical
// tree-sitter grammar) instead of a regex: alias, path, numeric and
// $prev tokens get their own classes, and half-typed refs degrade the
// same way the language server's analysis does.
import {
  ViewPlugin,
  Decoration,
  type DecorationSet,
  EditorView,
  type ViewUpdate,
} from "@codemirror/view";
import { RangeSetBuilder } from "@codemirror/state";
import { parser } from "@httui/lezer-refs";

const refMark = Decoration.mark({ class: "cm-reference-highlight" });
const tokenMarks: Record<string, Decoration> = {
  Identifier: Decoration.mark({ class: "cm-ref-name" }),
  Prev: Decoration.mark({ class: "cm-ref-prev" }),
  Number: Decoration.mark({ class: "cm-ref-index" }),
};

function buildDeco(view: EditorView): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>();
  for (const { from, to } of view.visibleRanges) {
    const text = view.state.sliceDoc(from, to);
    if (!text.includes("{{")) continue;
    parser.parse(text).iterate({
      enter(node) {
        if (node.name === "Ref") {
          builder.add(from + node.from, from + node.to, refMark);
        } else if (node.name === "Identifier") {
          // only the ref head (alias/env name) — path identifiers keep
          // the base highlight
          if (node.node.parent?.name === "RefBody") {
            builder.add(
              from + node.from,
              from + node.to,
              tokenMarks.Identifier,
            );
          }
        } else if (node.name in tokenMarks) {
          builder.add(from + node.from, from + node.to, tokenMarks[node.name]);
        }
      },
    });
  }
  return builder.finish();
}

const referenceHighlightPlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = buildDeco(view);
    }
    update(update: ViewUpdate) {
      if (update.docChanged || update.viewportChanged) {
        this.decorations = buildDeco(update.view);
      }
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
  ".cm-ref-name": {
    fontWeight: "600",
  },
  ".cm-ref-prev": {
    fontWeight: "600",
    fontStyle: "italic",
  },
  ".cm-ref-index": {
    opacity: "0.8",
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
