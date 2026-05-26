/**
 * Module-level registry of the currently focused CodeMirror view.
 *
 * Used by out-of-editor components (like `SchemaPanel`) that need to
 * dispatch edits into the last-focused editor without knowing about the
 * pane tree. `MarkdownEditor` calls `registerActiveEditor` on focus and
 * `unregisterActiveEditor` on blur / unmount.
 *
 * Why module-level rather than a React context:
 *  - Schema panel is a sibling of the pane tree, not a descendant — so a
 *    provider would need to be hoisted to AppShell and threaded down.
 *  - The registry is inherently imperative: "insert this text somewhere"
 *    is a side effect, not state React needs to observe.
 */
import { ViewPlugin } from "@codemirror/view";
import type { EditorView } from "@codemirror/view";
import type { Extension } from "@codemirror/state";

import { findDbBlocks } from "@/lib/codemirror/cm-db-block";
import { stringifyDbFenceInfo, type DbDialect } from "@/lib/blocks/db-fence";

let activeView: EditorView | null = null;

/** Call from the editor on focus or mount when focused. */
export function registerActiveEditor(view: EditorView): void {
  activeView = view;
}

/** Call from the editor on unmount. No-op if a different view is active. */
export function unregisterActiveEditor(view: EditorView): void {
  if (activeView === view) activeView = null;
}

export function getActiveEditor(): EditorView | null {
  return activeView;
}

/**
 * CM6 extension that keeps the active-editor registry in sync with focus
 * and tears everything down when the view is destroyed.
 *
 * The listeners are owned by a `ViewPlugin`, so CM guarantees `destroy()`
 * runs on view teardown (file switch keyed by `<CodeMirror
 * key={filePath}>`, vim toggle that recreates the view, unmount). There
 * `removeEventListener` is paired with the `addEventListener` and the
 * registry is cleared. This replaces the shell-side
 * `view.dom.addEventListener("focusin"/"focusout", …)` that was never
 * removed — recreating the view leaked stale listeners + closures and a
 * stray focus event could resurrect a dead view.
 */
export function activeEditorTracker(): Extension {
  return ViewPlugin.define((view) => {
    const onFocusIn = () => registerActiveEditor(view);
    const onFocusOut = () => unregisterActiveEditor(view);
    view.dom.addEventListener("focusin", onFocusIn);
    view.dom.addEventListener("focusout", onFocusOut);
    return {
      destroy() {
        view.dom.removeEventListener("focusin", onFocusIn);
        view.dom.removeEventListener("focusout", onFocusOut);
        unregisterActiveEditor(view);
      },
    };
  });
}

/**
 * Insert a SQL snippet into the active editor:
 *  - If the cursor is inside an existing db block body, replace the body
 *    with the snippet (most common case — user clicked a table while the
 *    block they want to populate is active).
 *  - Otherwise, insert a new fenced db block at the cursor position
 *    using `dialect` from the caller (usually derived from the selected
 *    connection driver).
 *
 * Returns `true` if the edit was dispatched, `false` if there's no
 * registered active editor.
 */
export function insertDbSnippetIntoActiveEditor(options: {
  snippet: string;
  dialect: DbDialect;
  connection?: string;
  alias?: string;
}): boolean {
  const view = activeView;
  if (!view) return false;

  const { snippet, dialect, connection, alias } = options;
  const doc = view.state.doc;
  const pos = view.state.selection.main.head;

  const blocks = findDbBlocks(doc);
  const inside = blocks.find((b) => pos >= b.bodyFrom && pos <= b.bodyTo);

  if (inside) {
    const replaceFrom = inside.bodyFrom;
    const replaceTo = inside.bodyTo;
    view.dispatch({
      changes: { from: replaceFrom, to: replaceTo, insert: snippet },
      selection: { anchor: replaceFrom + snippet.length },
    });
    view.focus();
    return true;
  }

  // Insert new block. Place it on its own line pair: a preceding newline if
  // the current line has content, a trailing newline so the next content
  // doesn't glue onto the closing fence.
  const info = stringifyDbFenceInfo({
    dialect,
    alias: alias ?? "db1",
    connection,
  });
  const line = doc.lineAt(pos);
  const atLineStart = pos === line.from;
  const lineIsEmpty = line.text.trim().length === 0;
  const leadingNewline = atLineStart || lineIsEmpty ? "" : "\n";
  const fence = `${leadingNewline}\`\`\`${info}\n${snippet}\n\`\`\`\n`;
  const cursor =
    pos + leadingNewline.length + 3 + info.length + 1 + snippet.length;
  view.dispatch({
    changes: { from: pos, insert: fence },
    selection: { anchor: cursor },
  });
  view.focus();
  return true;
}
