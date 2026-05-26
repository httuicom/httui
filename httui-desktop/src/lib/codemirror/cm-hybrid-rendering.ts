import { RangeSetBuilder, type Extension } from "@codemirror/state";
import {
  Decoration,
  type DecorationSet,
  EditorView,
  WidgetType,
} from "@codemirror/view";
import { syntaxTree } from "@codemirror/language";
import type { SyntaxNode } from "@lezer/common";

// ── Widgets ──────────────────────────────────────────────────────────────────

class BulletWidget extends WidgetType {
  toDOM() {
    const span = document.createElement("span");
    span.className = "cm-list-bullet";
    return span;
  }
  eq() {
    return true;
  }
}

class CheckboxWidget extends WidgetType {
  constructor(readonly checked: boolean) {
    super();
  }
  toDOM() {
    const cb = document.createElement("input");
    cb.type = "checkbox";
    cb.checked = this.checked;
    cb.className = "cm-task-checkbox";
    cb.disabled = true; // Read-only when rendered; interactive version can be added later
    return cb;
  }
  eq(other: CheckboxWidget) {
    return this.checked === other.checked;
  }
}

class ImageWidget extends WidgetType {
  constructor(
    readonly src: string,
    readonly alt: string,
  ) {
    super();
  }
  toDOM() {
    const img = document.createElement("img");
    img.src = this.src;
    img.alt = this.alt;
    img.className = "cm-image-widget";
    img.style.maxWidth = "100%";
    img.style.borderRadius = "6px";
    img.style.display = "block";
    img.style.margin = "4px 0";
    return img;
  }
  eq(other: ImageWidget) {
    return this.src === other.src;
  }
}

const FORMATTABLE_LANGS = new Set(["json"]);

function tryFormat(lang: string, content: string): string | null {
  if (lang === "json") {
    try {
      return JSON.stringify(JSON.parse(content), null, 2);
    } catch {
      return null;
    }
  }
  return null;
}

/** Find the content range (between fence markers) of the FencedCode enclosing `pos`. */
function findFencedCodeContent(
  view: EditorView,
  pos: number,
): { from: number; to: number } | null {
  const tree = syntaxTree(view.state);
  let node: SyntaxNode | null = tree.resolveInner(pos, 1);
  while (node && node.name !== "FencedCode") node = node.parent;
  if (!node) return null;
  const startLine = view.state.doc.lineAt(node.from).number;
  const endLine = view.state.doc.lineAt(node.to).number;
  if (endLine <= startLine) return null;
  return {
    from: view.state.doc.line(startLine + 1).from,
    to: view.state.doc.line(endLine - 1).to,
  };
}

class CodeToolbarWidget extends WidgetType {
  constructor(readonly lang: string) {
    super();
  }

  toDOM(view: EditorView): HTMLElement {
    const toolbar = document.createElement("div");
    toolbar.className = "cm-code-toolbar";
    toolbar.contentEditable = "false";

    const badge = document.createElement("span");
    badge.className = "cm-code-lang-badge";
    badge.textContent = (this.lang || "code").toUpperCase();
    toolbar.appendChild(badge);

    if (FORMATTABLE_LANGS.has(this.lang)) {
      const formatBtn = this.makeButton("Format", (btn) => {
        const range = findFencedCodeContent(view, view.posAtDOM(toolbar));
        if (!range) return;
        const content = view.state.doc.sliceString(range.from, range.to);
        const formatted = tryFormat(this.lang, content);
        if (formatted == null) {
          flash(btn, "Invalid", "error");
          return;
        }
        if (formatted !== content) {
          view.dispatch({
            changes: { from: range.from, to: range.to, insert: formatted },
          });
        }
      });
      toolbar.appendChild(formatBtn);
    }

    const copyBtn = this.makeButton("Copy", async (btn) => {
      const range = findFencedCodeContent(view, view.posAtDOM(toolbar));
      if (!range) return;
      const content = view.state.doc.sliceString(range.from, range.to);
      try {
        await navigator.clipboard.writeText(content);
        flash(btn, "Copied");
      } catch {
        flash(btn, "Failed", "error");
      }
    });
    toolbar.appendChild(copyBtn);

    return toolbar;
  }

  private makeButton(
    label: string,
    onClick: (btn: HTMLButtonElement) => void,
  ): HTMLButtonElement {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "cm-code-toolbar-btn";
    btn.textContent = label;
    btn.addEventListener("mousedown", (e) => {
      e.preventDefault();
    });
    btn.addEventListener("click", (e) => {
      e.preventDefault();
      e.stopPropagation();
      onClick(btn);
    });
    return btn;
  }

  eq(other: CodeToolbarWidget) {
    return this.lang === other.lang;
  }

  ignoreEvent() {
    return true;
  }
}

function flash(
  btn: HTMLButtonElement,
  msg: string,
  kind: "ok" | "error" = "ok",
) {
  const prev = btn.textContent;
  btn.textContent = msg;
  btn.dataset.flash = kind;
  window.setTimeout(() => {
    btn.textContent = prev;
    delete btn.dataset.flash;
  }, 1400);
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/** Get the set of line numbers that contain any part of the cursor/selection */
function getCursorLines(state: {
  doc: { lineAt(pos: number): { number: number } };
  selection: { ranges: readonly { from: number; to: number }[] };
}): Set<number> {
  const lines = new Set<number>();
  for (const range of state.selection.ranges) {
    const startLine = state.doc.lineAt(range.from).number;
    const endLine = state.doc.lineAt(range.to).number;
    for (let i = startLine; i <= endLine; i++) {
      lines.add(i);
    }
  }
  return lines;
}

/** Check if any line in a range overlaps with cursor lines */
function overlapsWithCursor(
  state: { doc: { lineAt(pos: number): { number: number } } },
  from: number,
  to: number,
  cursorLines: Set<number>,
): boolean {
  const startLine = state.doc.lineAt(from).number;
  const endLine = state.doc.lineAt(to).number;
  for (let i = startLine; i <= endLine; i++) {
    if (cursorLines.has(i)) return true;
  }
  return false;
}

// ── Heading line classes ─��───────────────────────────────────────────────────

const headingLineClasses: Record<string, Decoration> = {
  ATXHeading1: Decoration.line({ class: "cm-heading cm-heading-1" }),
  ATXHeading2: Decoration.line({ class: "cm-heading cm-heading-2" }),
  ATXHeading3: Decoration.line({ class: "cm-heading cm-heading-3" }),
  ATXHeading4: Decoration.line({ class: "cm-heading cm-heading-4" }),
  ATXHeading5: Decoration.line({ class: "cm-heading cm-heading-5" }),
  ATXHeading6: Decoration.line({ class: "cm-heading cm-heading-6" }),
};

// ── Main decoration builder ──────────────────────────────────────────────────

function buildDecorations(view: EditorView): DecorationSet {
  const { state } = view;
  const cursorLines = getCursorLines(state);
  const decorations: { from: number; to: number; deco: Decoration }[] = [];
  const tree = syntaxTree(state);

  tree.iterate({
    enter(node: SyntaxNode) {
      const name = node.type.name;

      // ── Headings (Tier 2: always styled, reveal # on cursor) ──
      if (name.startsWith("ATXHeading") && headingLineClasses[name]) {
        // Always apply heading line class — keeps font size/weight even on cursor line
        const line = state.doc.lineAt(node.from);
        decorations.push({
          from: line.from,
          to: line.from,
          deco: headingLineClasses[name],
        });
        // Only hide # marks when cursor is NOT on this line
        if (!overlapsWithCursor(state, node.from, node.to, cursorLines)) {
          const headerMark = node.node.getChild("HeaderMark");
          if (headerMark) {
            const hideEnd = Math.min(headerMark.to + 1, node.to);
            decorations.push({
              from: headerMark.from,
              to: hideEnd,
              deco: Decoration.replace({}),
            });
          }
        }
      }

      // ── Bold (StrongEmphasis) ──
      if (name === "StrongEmphasis") {
        if (!overlapsWithCursor(state, node.from, node.to, cursorLines)) {
          // Hide opening **
          decorations.push({
            from: node.from,
            to: node.from + 2,
            deco: Decoration.replace({}),
          });
          // Hide closing **
          decorations.push({
            from: node.to - 2,
            to: node.to,
            deco: Decoration.replace({}),
          });
          // Apply bold styling
          decorations.push({
            from: node.from + 2,
            to: node.to - 2,
            deco: Decoration.mark({ class: "cm-strong" }),
          });
        }
      }

      // ── Italic (Emphasis) ──
      if (name === "Emphasis") {
        if (!overlapsWithCursor(state, node.from, node.to, cursorLines)) {
          decorations.push({
            from: node.from,
            to: node.from + 1,
            deco: Decoration.replace({}),
          });
          decorations.push({
            from: node.to - 1,
            to: node.to,
            deco: Decoration.replace({}),
          });
          decorations.push({
            from: node.from + 1,
            to: node.to - 1,
            deco: Decoration.mark({ class: "cm-em" }),
          });
        }
      }

      // ── Strikethrough ──
      if (name === "Strikethrough") {
        if (!overlapsWithCursor(state, node.from, node.to, cursorLines)) {
          decorations.push({
            from: node.from,
            to: node.from + 2,
            deco: Decoration.replace({}),
          });
          decorations.push({
            from: node.to - 2,
            to: node.to,
            deco: Decoration.replace({}),
          });
          decorations.push({
            from: node.from + 2,
            to: node.to - 2,
            deco: Decoration.mark({ class: "cm-strikethrough" }),
          });
        }
      }

      // ── Inline code ──
      if (name === "InlineCode") {
        if (!overlapsWithCursor(state, node.from, node.to, cursorLines)) {
          // Hide opening `
          decorations.push({
            from: node.from,
            to: node.from + 1,
            deco: Decoration.replace({}),
          });
          // Hide closing `
          decorations.push({
            from: node.to - 1,
            to: node.to,
            deco: Decoration.replace({}),
          });
          decorations.push({
            from: node.from + 1,
            to: node.to - 1,
            deco: Decoration.mark({ class: "cm-inline-code" }),
          });
        }
      }

      // ── Links [text](url) ──
      if (name === "Link") {
        if (!overlapsWithCursor(state, node.from, node.to, cursorLines)) {
          const urlNode = node.node.getChild("URL");
          const linkMarkOpen = node.node.getChild("LinkMark");
          if (urlNode && linkMarkOpen) {
            // Get link text content
            const textStart = linkMarkOpen.to; // after [
            const textEnd = urlNode.from - 2; // before ](
            if (textStart < textEnd) {
              // Replace entire link with just styled text
              decorations.push({
                from: node.from,
                to: textStart,
                deco: Decoration.replace({}),
              });
              decorations.push({
                from: textStart,
                to: textEnd,
                deco: Decoration.mark({ class: "cm-link" }),
              });
              decorations.push({
                from: textEnd,
                to: node.to,
                deco: Decoration.replace({}),
              });
            }
          }
        }
      }

      // ── Images ![alt](url) ──
      if (name === "Image") {
        if (!overlapsWithCursor(state, node.from, node.to, cursorLines)) {
          const urlNode = node.node.getChild("URL");
          if (urlNode) {
            const url = state.doc.sliceString(urlNode.from, urlNode.to);
            const altMark = node.node.getChild("LinkMark");
            let alt = "";
            if (altMark) {
              // Text between ![ and ]
              alt = state.doc.sliceString(altMark.to, urlNode.from - 2);
            }
            decorations.push({
              from: node.from,
              to: node.to,
              deco: Decoration.replace({
                widget: new ImageWidget(url, alt),
                block: false,
              }),
            });
          }
        }
      }

      // ── Horizontal rule (Tier 2: always a styled line) ──
      if (name === "HorizontalRule") {
        const line = state.doc.lineAt(node.from);
        const onCursor = overlapsWithCursor(
          state,
          node.from,
          node.to,
          cursorLines,
        );
        decorations.push({
          from: line.from,
          to: line.from,
          deco: Decoration.line({
            class: onCursor ? "cm-hr-line cm-hr-editing" : "cm-hr-line",
          }),
        });
      }

      // ── List bullets (- or *) ──
      if (name === "ListMark") {
        if (!overlapsWithCursor(state, node.from, node.to, cursorLines)) {
          const text = state.doc.sliceString(node.from, node.to).trim();
          // Only replace unordered list markers (- or *), not ordered (1.)
          if (text === "-" || text === "*") {
            // Check if this is NOT a task list item (task items have TaskMarker)
            const parent = node.node.parent;
            const hasTaskMarker = parent?.getChild("TaskMarker");
            if (!hasTaskMarker) {
              const hideEnd = Math.min(
                node.to + 1,
                state.doc.lineAt(node.from).to,
              );
              decorations.push({
                from: node.from,
                to: hideEnd,
                deco: Decoration.replace({
                  widget: new BulletWidget(),
                }),
              });
            }
          }
        }
      }

      // ── Task list items — hide the "- " before the checkbox ──
      if (name === "TaskMarker") {
        if (!overlapsWithCursor(state, node.from, node.to, cursorLines)) {
          // Hide the ListMark (- ) before the task marker
          const listMark = node.node.parent?.getChild("ListMark");
          if (listMark) {
            const hideEnd = Math.min(
              listMark.to + 1,
              state.doc.lineAt(listMark.from).to,
            );
            decorations.push({
              from: listMark.from,
              to: hideEnd,
              deco: Decoration.replace({}),
            });
          }

          const text = state.doc.sliceString(node.from, node.to);
          const checked = text.includes("x") || text.includes("X");
          decorations.push({
            from: node.from,
            to: node.to,
            deco: Decoration.replace({
              widget: new CheckboxWidget(checked),
            }),
          });
        }
      }

      // ── Fenced code blocks (```lang ... ```) ──
      // Executable blocks (http/db) are handled separately by their own
      // CM6 extensions (`cm-http-block.tsx` / `cm-db-block.tsx`).
      if (name === "FencedCode") {
        const codeInfo = node.node.getChild("CodeInfo");
        const lang = codeInfo
          ? state.doc.sliceString(codeInfo.from, codeInfo.to).trim()
          : "";
        // CodeInfo covers the entire info string (lang + key=value attrs).
        // Only the first token is the dialect; use it for the executable check.
        const langToken = lang.split(/\s+/)[0];
        const isExecutable = /^(http|db(?:-[\w:-]+)?)$/.test(langToken);
        if (!isExecutable) {
          const startLine = state.doc.lineAt(node.from).number;
          const endLine = state.doc.lineAt(node.to).number;
          const onCursor = overlapsWithCursor(
            state,
            node.from,
            node.to,
            cursorLines,
          );
          // Visually collapsed range when cursor is away: [startLine + 1, endLine - 1]
          const contentStart = onCursor ? startLine : startLine + 1;
          const contentEnd = onCursor ? endLine : endLine - 1;
          for (let i = startLine; i <= endLine; i++) {
            const line = state.doc.line(i);
            let cls = "cm-fenced-code-line";
            if (i === contentStart) cls += " cm-fenced-code-open";
            if (i === contentEnd) cls += " cm-fenced-code-close";
            if (!onCursor && (i === startLine || i === endLine)) {
              cls = "cm-fenced-code-fence-hidden";
            }
            decorations.push({
              from: line.from,
              to: line.from,
              deco: Decoration.line({ class: cls }),
            });
          }
          // When cursor is away, hide fence marker lines (replace content).
          if (!onCursor) {
            const openLine = state.doc.line(startLine);
            if (openLine.length > 0) {
              decorations.push({
                from: openLine.from,
                to: openLine.to,
                deco: Decoration.replace({}),
              });
            }
            if (endLine > startLine) {
              const closeLine = state.doc.line(endLine);
              if (closeLine.length > 0) {
                decorations.push({
                  from: closeLine.from,
                  to: closeLine.to,
                  deco: Decoration.replace({}),
                });
              }
            }
          }
          // Floating toolbar (badge + Copy + Format) anchored to the first visible
          // content line — always shown so Copy/Format remain accessible while editing.
          if (contentStart <= endLine) {
            const toolbarAnchor = state.doc.line(contentStart).from;
            decorations.push({
              from: toolbarAnchor,
              to: toolbarAnchor,
              deco: Decoration.widget({
                widget: new CodeToolbarWidget(lang),
                side: -1,
              }),
            });
          }
        }
      }

      // ── Blockquote markers (Tier 2: always styled, reveal > on cursor) ──
      if (name === "QuoteMark") {
        const line = state.doc.lineAt(node.from);
        decorations.push({
          from: line.from,
          to: line.from,
          deco: Decoration.line({ class: "cm-blockquote-line" }),
        });
        if (!overlapsWithCursor(state, node.from, node.to, cursorLines)) {
          // Hide the > character
          const hideEnd = Math.min(node.to + 1, line.to);
          decorations.push({
            from: node.from,
            to: hideEnd,
            deco: Decoration.replace({}),
          });
        }
      }
    },
  });

  // Sort by from position, then by whether it's a line decoration (from === to means line or point)
  decorations.sort((a, b) => a.from - b.from || a.to - b.to);

  const builder = new RangeSetBuilder<Decoration>();
  for (const { from, to, deco } of decorations) {
    builder.add(from, to, deco);
  }
  return builder.finish();
}

// ── Extension ────────────────────────────────────────────────────────────────

import { ViewPlugin, type ViewUpdate } from "@codemirror/view";

const hybridPlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    lastCursorLine: number;

    constructor(view: EditorView) {
      this.decorations = buildDecorations(view);
      this.lastCursorLine = view.state.doc.lineAt(
        view.state.selection.main.head,
      ).number;
    }

    update(update: ViewUpdate) {
      const currentLine = update.state.doc.lineAt(
        update.state.selection.main.head,
      ).number;
      const cursorLineMoved = currentLine !== this.lastCursorLine;

      if (cursorLineMoved) {
        this.decorations = buildDecorations(update.view);
        this.lastCursorLine = currentLine;
      } else if (update.docChanged) {
        this.decorations = this.decorations.map(update.changes);
      }
    }
  },
  {
    decorations: (v) => v.decorations,
  },
);

// ── Theme ───────────────���────────────────────────────────────────────────────

const hybridTheme = EditorView.theme({
  // Headings — clean Notion-like style, no margins (avoid layout shift on cursor move)
  ".cm-heading-1": {
    fontSize: "1.875em",
    fontWeight: "700",
    lineHeight: "1.4",
    letterSpacing: "-0.02em",
  },
  ".cm-heading-2": {
    fontSize: "1.5em",
    fontWeight: "600",
    lineHeight: "1.4",
    letterSpacing: "-0.01em",
  },
  ".cm-heading-3": { fontSize: "1.25em", fontWeight: "600", lineHeight: "1.4" },
  ".cm-heading-4": { fontSize: "1.1em", fontWeight: "600", lineHeight: "1.4" },
  ".cm-heading-5": { fontSize: "1em", fontWeight: "600", lineHeight: "1.4" },
  ".cm-heading-6": {
    fontSize: "0.9em",
    fontWeight: "600",
    lineHeight: "1.4",
    color: "var(--chakra-colors-fg-muted)",
  },

  // Inline formatting
  ".cm-strong": { fontWeight: "600" },
  ".cm-em": { fontStyle: "italic" },
  ".cm-strikethrough": { textDecoration: "line-through", opacity: "0.6" },
  ".cm-inline-code": {
    backgroundColor: "var(--chakra-colors-bg-subtle)",
    borderRadius: "4px",
    padding: "2px 6px",
    fontFamily: "var(--chakra-fonts-mono)",
    fontSize: "0.85em",
    color: "var(--chakra-colors-pink-500)",
  },

  // Links — Notion-style subtle underline
  ".cm-link": {
    color: "var(--chakra-colors-fg)",
    textDecoration: "underline",
    textDecorationColor: "var(--chakra-colors-fg-subtle)",
    textUnderlineOffset: "2px",
    cursor: "pointer",
    "&:hover": {
      textDecorationColor: "var(--chakra-colors-fg)",
    },
  },

  // Horizontal rule — full-width line, text sits on top when editing
  ".cm-hr-line": {
    position: "relative",
    color: "transparent",
  },
  ".cm-hr-line::after": {
    content: '""',
    position: "absolute",
    left: "0",
    right: "0",
    top: "50%",
    borderTop: "1px solid var(--chakra-colors-border)",
    pointerEvents: "none",
  },
  ".cm-hr-line.cm-hr-editing": {
    color: "var(--chakra-colors-fg-subtle)",
  },
  ".cm-hr-line.cm-hr-editing::after": {
    display: "none",
  },

  // Task checkboxes — larger, Notion-style
  ".cm-task-checkbox": {
    marginRight: "8px",
    verticalAlign: "middle",
    width: "16px",
    height: "16px",
    accentColor: "var(--chakra-colors-blue-500)",
    cursor: "pointer",
  },

  // List bullet — Notion-style
  ".cm-list-bullet": {
    display: "inline-block",
    width: "6px",
    height: "6px",
    borderRadius: "50%",
    backgroundColor: "var(--chakra-colors-fg)",
    marginRight: "10px",
    verticalAlign: "middle",
    opacity: "0.6",
  },

  // Blockquotes — Notion-style
  ".cm-blockquote-line": {
    borderLeft: "3px solid var(--chakra-colors-fg)",
    paddingLeft: "16px",
    fontStyle: "italic",
    color: "var(--chakra-colors-fg-subtle)",
  },

  // Fenced code blocks — unified visual container across lines
  ".cm-fenced-code-line": {
    backgroundColor: "var(--chakra-colors-bg-subtle)",
    fontFamily: "var(--chakra-fonts-mono)",
    fontSize: "0.85em",
    paddingLeft: "16px",
    paddingRight: "16px",
    borderLeft: "1px solid var(--chakra-colors-border)",
    borderRight: "1px solid var(--chakra-colors-border)",
  },
  ".cm-fenced-code-open": {
    borderTop: "1px solid var(--chakra-colors-border)",
    borderTopLeftRadius: "6px",
    borderTopRightRadius: "6px",
    paddingTop: "6px",
    position: "relative",
  },
  ".cm-fenced-code-close": {
    borderBottom: "1px solid var(--chakra-colors-border)",
    borderBottomLeftRadius: "6px",
    borderBottomRightRadius: "6px",
    paddingBottom: "6px",
  },
  // Fence lines (``` ... ```) shown only when cursor is inside the block
  ".cm-fenced-code-fence-hidden": {
    height: "0 !important",
    padding: "0 !important",
    margin: "0 !important",
    overflow: "hidden !important",
    fontSize: "0 !important",
    lineHeight: "0 !important",
    border: "none !important",
  },
  // Floating toolbar (badge + action buttons) anchored to the top-right of the block
  ".cm-code-toolbar": {
    position: "absolute",
    right: "8px",
    top: "4px",
    display: "flex",
    alignItems: "center",
    gap: "4px",
    zIndex: "2",
    fontFamily: "var(--chakra-fonts-body)",
    userSelect: "none",
  },
  ".cm-code-toolbar .cm-code-lang-badge": {
    fontSize: "10px",
    fontFamily: "var(--chakra-fonts-mono)",
    letterSpacing: "0.04em",
    color: "var(--chakra-colors-fg-subtle)",
    backgroundColor: "var(--chakra-colors-bg)",
    padding: "2px 6px",
    borderRadius: "4px",
    border: "1px solid var(--chakra-colors-border)",
    pointerEvents: "none",
  },
  ".cm-code-toolbar-btn": {
    fontSize: "10px",
    fontFamily: "inherit",
    color: "var(--chakra-colors-fg-muted)",
    backgroundColor: "var(--chakra-colors-bg)",
    padding: "2px 8px",
    borderRadius: "4px",
    border: "1px solid var(--chakra-colors-border)",
    cursor: "pointer",
    lineHeight: "1.4",
    transition: "color 120ms, background-color 120ms, border-color 120ms",
  },
  ".cm-code-toolbar-btn:hover": {
    color: "var(--chakra-colors-fg)",
    borderColor: "var(--chakra-colors-fg-subtle)",
  },
  ".cm-code-toolbar-btn[data-flash='ok']": {
    color: "var(--chakra-colors-green-400)",
    borderColor: "var(--chakra-colors-green-400)",
  },
  ".cm-code-toolbar-btn[data-flash='error']": {
    color: "var(--chakra-colors-red-400)",
    borderColor: "var(--chakra-colors-red-400)",
  },
});

/** Hybrid markdown rendering extension — renders markdown when cursor is away, shows raw on cursor line */
export function hybridRendering(): Extension {
  return [hybridPlugin, hybridTheme];
}
