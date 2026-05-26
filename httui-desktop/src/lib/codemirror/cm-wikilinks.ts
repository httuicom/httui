import { RangeSetBuilder, type Extension } from "@codemirror/state";
import {
  Decoration,
  type DecorationSet,
  EditorView,
  ViewPlugin,
  type ViewUpdate,
  WidgetType,
} from "@codemirror/view";
import {
  type CompletionContext,
  type CompletionResult,
  type Completion,
} from "@codemirror/autocomplete";

// ── Types ────────────────────────────────────────────────────────────────────

interface WikilinkMatch {
  from: number; // start of [[
  to: number; // end of ]]
  target: string;
  label: string;
}

interface WikilinkOptions {
  getFiles: () => { name: string; path: string }[];
  onNavigate: (target: string) => void;
}

// ── Regex ────────────────────────────────────────────────────────────────────

const WIKILINK_RE = /\[\[([^\]|]+)(?:\|([^\]]+))?\]\]/g;

function findWikilinks(text: string, offset: number): WikilinkMatch[] {
  const matches: WikilinkMatch[] = [];
  let m: RegExpExecArray | null;
  WIKILINK_RE.lastIndex = 0;
  while ((m = WIKILINK_RE.exec(text)) !== null) {
    const target = m[1].trim();
    const label = m[2]?.trim() || target.replace(/\.md$/, "");
    matches.push({
      from: offset + m.index,
      to: offset + m.index + m[0].length,
      target,
      label,
    });
  }
  return matches;
}

// ── Widget ───────────────────────────────────────────────────────────────────

class WikilinkWidget extends WidgetType {
  constructor(
    readonly target: string,
    readonly label: string,
    readonly onNavigate: (target: string) => void,
  ) {
    super();
  }

  toDOM(): HTMLElement {
    const span = document.createElement("span");
    span.className = "cm-wikilink";
    span.textContent = this.label;
    span.title = this.target;
    span.addEventListener("click", (e) => {
      e.preventDefault();
      e.stopPropagation();
      this.onNavigate(this.target);
    });
    return span;
  }

  eq(other: WikilinkWidget): boolean {
    return this.target === other.target && this.label === other.label;
  }

  ignoreEvent(): boolean {
    return false; // Let click events through to our handler
  }
}

// ── Decoration plugin ────────────────────────────────────────────────────────

function buildWikilinkDecorations(
  view: EditorView,
  onNavigate: (target: string) => void,
): DecorationSet {
  const { state } = view;
  const builder = new RangeSetBuilder<Decoration>();

  // Get cursor line numbers
  const cursorLines = new Set<number>();
  for (const range of state.selection.ranges) {
    const startLine = state.doc.lineAt(range.from).number;
    const endLine = state.doc.lineAt(range.to).number;
    for (let i = startLine; i <= endLine; i++) {
      cursorLines.add(i);
    }
  }

  // Scan visible lines for wikilinks
  for (let i = 1; i <= state.doc.lines; i++) {
    // If cursor is on this line, show raw [[target]]
    if (cursorLines.has(i)) continue;

    const line = state.doc.line(i);
    const matches = findWikilinks(line.text, line.from);
    for (const match of matches) {
      builder.add(
        match.from,
        match.to,
        Decoration.replace({
          widget: new WikilinkWidget(match.target, match.label, onNavigate),
        }),
      );
    }
  }

  return builder.finish();
}

function createWikilinkPlugin(onNavigate: (target: string) => void) {
  return ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;
      constructor(view: EditorView) {
        this.decorations = buildWikilinkDecorations(view, onNavigate);
      }
      update(update: ViewUpdate) {
        if (update.docChanged || update.selectionSet) {
          this.decorations = buildWikilinkDecorations(update.view, onNavigate);
        }
      }
    },
    { decorations: (v) => v.decorations },
  );
}

// ── Autocomplete ─────────────────────────────────────────────────────────────

function createWikilinkCompletion(
  getFiles: () => { name: string; path: string }[],
) {
  return (context: CompletionContext): CompletionResult | null => {
    // Match [[ followed by optional text
    const before = context.state.doc.sliceString(
      Math.max(0, context.pos - 50),
      context.pos,
    );
    const match = before.match(/\[\[([^\]|]*)$/);
    if (!match) return null;

    const query = match[1].toLowerCase();
    const from = context.pos - match[1].length;
    const files = getFiles();

    const filtered = query
      ? files.filter((f) => f.name.toLowerCase().includes(query))
      : files;

    const options: Completion[] = filtered.slice(0, 10).map((f) => {
      const label = f.name.replace(/\.md$/, "");
      return {
        label,
        apply: (
          view: EditorView,
          _completion: Completion,
          from: number,
          to: number,
        ) => {
          // Replace the text after [[ with target]] (keep the opening [[)
          const insert = `${f.path}|${label}]]`;
          view.dispatch({
            changes: { from, to, insert },
          });
        },
      };
    });

    return { from, options, filter: false };
  };
}

// ── Theme ────────────────────────────────────────────────────────────────────

const wikilinkTheme = EditorView.theme({
  ".cm-wikilink": {
    color: "var(--chakra-colors-blue-500)",
    cursor: "pointer",
    textDecoration: "underline",
    textDecorationStyle: "dotted",
    textDecorationColor: "var(--chakra-colors-blue-300)",
    "&:hover": {
      textDecorationStyle: "solid",
    },
  },
});

// ── Export ────────────────────────────────────────────────────────────────────

/** Export the completion source factory for combining with other sources */
export { createWikilinkCompletion };

/** Wikilink extension for CM6 — decorations + click navigation (autocomplete handled externally) */
export function wikilinks(options: WikilinkOptions): Extension {
  return [createWikilinkPlugin(options.onNavigate), wikilinkTheme];
}
