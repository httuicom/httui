import {
  ViewPlugin,
  Decoration,
  type DecorationSet,
  EditorView,
  type ViewUpdate,
  MatchDecorator,
} from "@codemirror/view";

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
