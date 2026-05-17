// V2 / cenário 4.5 — DocHeader inline as a CM6 block widget.
//
// Mounts the React `DocHeaderShell` at the very top of the document via
// `Decoration.widget` (block, side: -1) at offset 0. When the doc has a
// YAML frontmatter (`^---\n…\n---\n`), the corresponding range is hidden
// with `Decoration.replace` and added to `EditorView.atomicRanges` so the
// cursor skips over it.
//
// Mirrors the portal-registry pattern from `cm-http-block.tsx` —
// simplified to a single slot per editor view (each editor has at most
// one DocHeader). React mounts via `DocHeaderWidgetPortal.tsx`. The
// portal registry + frontmatter-sync helpers live in the sibling
// `cm-doc-header-state.ts` so this file stays focused on CM6 plumbing.

import {
  EditorSelection,
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
import { getCM } from "@replit/codemirror-vim";

import {
  bindView,
  FRONTMATTER_REWRITE,
  getDocHeaderEntries,
  registerContainer,
  setFieldForInstance,
  syncEntryFrontmatter,
  unregisterContainer,
  type FrontmatterRange,
} from "./cm-doc-header-state";

// Re-export the public registry surface so existing call sites keep
// importing from `cm-doc-header` without churn.
export {
  dispatchDocReplace,
  getDocHeaderEntries,
  getDocHeaderPortalVersion,
  registerDocHeaderTitleInput,
  returnFocusToBody,
  subscribeToDocHeaderPortals,
  type DocHeaderEntry,
  type FrontmatterRange,
} from "./cm-doc-header-state";

// Vim ownership guard — when vim is in normal / visual mode, hjkl /
// arrows belong to vim and we MUST NOT intercept them. In insert mode
// (or when vim is off entirely) the editor is in "regular text mode"
// and our ArrowUp-at-body-start handler is fair game.
function vimOwnsMotion(view: EditorView): boolean {
  const cm = getCM(view);
  const v = cm?.state.vim;
  if (!v) return false;
  return !v.insertMode;
}

// ───── Frontmatter detection ─────

/**
 * Detect a YAML frontmatter block at the very top of the document.
 *
 * Returns `null` unless the doc starts with `---\n` (or `---\r\n`) AND has
 * a closing `---` line further down. The close line must be exactly `---`
 * after trimming. Anything else (no fence, fence in the middle of the doc,
 * unclosed fence) returns null.
 *
 * The returned `to` offset is positioned right after the `\n` that
 * terminates the closing fence, so the replace range cleanly removes the
 * whole block (no orphaned blank line).
 */
export function findFrontmatterRange(doc: CMText): FrontmatterRange | null {
  if (doc.lines === 0) return null;
  const first = doc.line(1);
  if (first.text !== "---") return null;

  for (let n = 2; n <= doc.lines; n++) {
    const line = doc.line(n);
    if (line.text === "---") {
      // `line.to` is the offset of the line's terminator (or doc end). We
      // want to also swallow the newline that ends the closing fence so
      // the body cursor lands on the body's first column. When there's
      // no trailing newline (frontmatter is the last thing in the doc),
      // `line.to` already equals doc length.
      const docLen = doc.length;
      const swallowNewline = line.to < docLen ? 1 : 0;
      return { from: 0, to: line.to + swallowNewline };
    }
  }
  return null;
}

// ───── Per-extension instance IDs ─────

let nextInstanceId = 0;
function createInstanceId(): string {
  nextInstanceId += 1;
  return `doc_header_${nextInstanceId}`;
}

// ───── Widget ─────

class DocHeaderWidget extends WidgetType {
  constructor(
    readonly instanceId: string,
    readonly hasFrontmatter: boolean,
  ) {
    super();
  }

  toDOM(view: EditorView): HTMLElement {
    const div = document.createElement("div");
    div.className = "cm-doc-header-portal";
    div.setAttribute("data-doc-header-id", this.instanceId);
    div.contentEditable = "false";
    registerContainer(this.instanceId, div, this.hasFrontmatter);
    bindView(this.instanceId, view, div);
    // Seed the live frontmatter on first mount so the portal renders
    // with the correct tags / checklist immediately, without waiting
    // for a doc change.
    syncEntryFrontmatter(this.instanceId, view.state.doc.toString());
    return div;
  }

  updateDOM(dom: HTMLElement, view: EditorView): boolean {
    registerContainer(this.instanceId, dom, this.hasFrontmatter);
    bindView(this.instanceId, view, dom);
    syncEntryFrontmatter(this.instanceId, view.state.doc.toString());
    return true;
  }

  destroy(): void {
    unregisterContainer(this.instanceId);
  }

  eq(other: DocHeaderWidget): boolean {
    return (
      this.instanceId === other.instanceId &&
      this.hasFrontmatter === other.hasFrontmatter
    );
  }

  ignoreEvent(): boolean {
    // CM6 must NOT process events that originate inside the widget —
    // otherwise a click on the title input is intercepted and the
    // editor's cursor jumps somewhere in the doc, stealing focus
    // before React's `<input>` gets it. The HTTP/DB widgets follow
    // the same convention for the same reason.
    return true;
  }

  get estimatedHeight(): number {
    // The shell is a card with breadcrumb + title + meta + abstract. The
    // exact height is decided by React; this seed only affects scroll
    // sizing during the brief window before the first measure.
    return 220;
  }
}

// ───── Decorations ─────

function buildDocHeaderDecorations(
  state: EditorState,
  instanceId: string,
): { decorations: DecorationSet; range: FrontmatterRange | null } {
  const range = findFrontmatterRange(state.doc);
  const builder = new RangeSetBuilder<Decoration>();

  // Block widget at offset 0 with side: -1 — appears BEFORE the first
  // line. Stays in place even when the frontmatter is hidden (the widget
  // is a separate decoration, independent of the replace below).
  builder.add(
    0,
    0,
    Decoration.widget({
      widget: new DocHeaderWidget(instanceId, range !== null),
      block: true,
      side: -1,
    }),
  );

  if (range !== null) {
    builder.add(
      range.from,
      range.to,
      Decoration.replace({
        block: true,
        inclusive: true,
      }),
    );
  }

  return { decorations: builder.finish(), range };
}

// ───── Public extension factory ─────

export interface DocHeaderExtensionHandle {
  /** The CM6 extension to add to the editor's `extensions` array. */
  extension: Extension;
  /** Unique ID assigned to this extension instance. The React portal
   *  uses it to find its corresponding entry in the global registry —
   *  needed so multiple concurrent editors each render the right
   *  DocHeader. */
  instanceId: string;
}

export function createDocHeaderExtension(): DocHeaderExtensionHandle {
  const instanceId = createInstanceId();

  const field = StateField.define<{
    decorations: DecorationSet;
    range: FrontmatterRange | null;
  }>({
    create(state) {
      return buildDocHeaderDecorations(state, instanceId);
    },
    update(value, tr) {
      if (!tr.docChanged) return value;
      return buildDocHeaderDecorations(tr.state, instanceId);
    },
    provide: (f) => [
      EditorView.decorations.from(f, (v) => v.decorations),
      // Atomic-range provider — cursor skips the hidden frontmatter on
      // arrow / motion. Without this, programmatic selection (and some
      // edge-case CM6 motion paths) could land inside the replaced range.
      EditorView.atomicRanges.of((view) => {
        const value = view.state.field(f, false);
        if (!value || !value.range) return Decoration.none;
        const { from, to } = value.range;
        const builder = new RangeSetBuilder<Decoration>();
        builder.add(from, to, Decoration.mark({}));
        return builder.finish();
      }),
    ],
  });

  setFieldForInstance(instanceId, field);

  // Track:
  //   1. the last cursor offset inside the body — leaving + re-entering
  //      restores the position (M3),
  //   2. the live frontmatter parse — pushed into the entry so the
  //      React portal sees fresh tags / checklist / abstract / title
  //      without needing the upstream `content` prop to change. (The
  //      pane store's editorContents is a non-reactive Map, so React
  //      doesn't otherwise re-render on body keystrokes.)
  const updateListener = EditorView.updateListener.of((u) => {
    if (u.docChanged) {
      syncEntryFrontmatter(instanceId, u.state.doc.toString());
    }
    if (!u.selectionSet && !u.viewportChanged) return;
    const value = u.state.field(field, false);
    if (!value) return;
    const head = u.state.selection.main.head;
    const bodyStart = value.range?.to ?? 0;
    if (head < bodyStart) return;
    syncBodyOffset(instanceId, head);
  });

  // ArrowUp at body start → focus the title input. Bails when vim
  // owns motion (normal / visual mode). High precedence so we run
  // before the doc-line nav keymap in MarkdownEditor.
  const navKeymap: KeyBinding[] = [
    {
      key: "ArrowUp",
      run: (view) => {
        if (vimOwnsMotion(view)) return false;
        const sel = view.state.selection.main;
        if (!sel.empty) return false;
        const value = view.state.field(field, false);
        if (!value) return false;
        const bodyStart = value.range?.to ?? 0;
        // Treat the line that starts at `bodyStart` as the first body
        // line. The cursor is on it iff `head` falls within the
        // doc-line that begins at `bodyStart`.
        const headLine = view.state.doc.lineAt(sel.head);
        const bodyLine = view.state.doc.lineAt(bodyStart);
        if (headLine.number !== bodyLine.number) return false;
        const input = getTitleInputFor(instanceId);
        if (!input) return false;
        // Remember where the cursor was so the input's
        // back-to-body action can restore it.
        syncBodyOffset(instanceId, sel.head);
        input.focus();
        // Most browsers preserve the caret on a refocus. Move it to
        // the end of the value for predictable Notion-like UX.
        const len = input.value.length;
        input.setSelectionRange(len, len);
        return true;
      },
    },
    {
      // Cmd/Ctrl-A — clip "select all" to the body range so a follow-
      // up Delete doesn't try to chew through the hidden frontmatter
      // (the guard would block it entirely, surprising the user). With
      // no frontmatter, fall through to the default behavior.
      key: "Mod-a",
      run: (view) => {
        if (vimOwnsMotion(view)) return false;
        const value = view.state.field(field, false);
        if (!value || !value.range) return false;
        const bodyStart = value.range.to;
        const docEnd = view.state.doc.length;
        if (bodyStart >= docEnd) return false;
        view.dispatch({
          selection: EditorSelection.range(bodyStart, docEnd),
        });
        return true;
      },
    },
  ];

  // Block changes that would touch the hidden frontmatter range. This
  // catches Backspace / Delete / cut at the body's first line — the
  // cursor sits at `range.to`, so a default backspace would otherwise
  // delete the trailing newline of the closing fence and silently
  // start eating the YAML upward. Only programmatic header rewrites
  // (annotated `FRONTMATTER_REWRITE`) are allowed through.
  const guard = EditorState.transactionFilter.of((tr) => {
    if (!tr.docChanged) return tr;
    if (tr.annotation(FRONTMATTER_REWRITE)) return tr;
    const value = tr.startState.field(field, false);
    if (!value || !value.range) return tr;
    const rFrom = value.range.from;
    const rTo = value.range.to;
    let touches = false;
    tr.changes.iterChanges((from, to) => {
      if (touches) return;
      // Standard half-open range overlap check. Anything from the
      // first frontmatter byte through (but not including) the body
      // start counts as a hit.
      if (from < rTo && to > rFrom) touches = true;
    });
    if (touches) {
      // Drop the change but preserve the selection update if any —
      // returning an empty array would also cancel selection moves the
      // user might want (Backspace selecting the prev char visually,
      // for instance). The simplest safe call is to return an empty
      // transaction spec, which discards everything.
      return [];
    }

    // After a clean change: if the resulting doc ends right at the
    // frontmatter close (`rTo`), the body is empty and CM6 has no
    // line to anchor the caret on — typing still works but the user
    // sees an invisible cursor (Cmd-A + Delete reproduces this).
    // Pad with `\n` so the body has one empty line for the caret.
    if (tr.changes.newLength === rTo) {
      return [
        tr,
        {
          changes: { from: rTo, insert: "\n" },
          selection: EditorSelection.cursor(rTo),
          annotations: FRONTMATTER_REWRITE.of(true),
        },
      ];
    }
    return tr;
  });

  return {
    extension: [field, guard, updateListener, Prec.high(keymap.of(navKeymap))],
    instanceId,
  };
}

// Thin accessors that hide the registry's private map from this file.
// Kept narrow so the CM6 wiring stays decoupled from the registry
// internals.

function getTitleInputFor(instanceId: string): HTMLInputElement | null {
  return getDocHeaderEntries().get(instanceId)?.titleInput ?? null;
}

function syncBodyOffset(instanceId: string, offset: number) {
  const entry = getDocHeaderEntries().get(instanceId);
  if (entry) entry.lastBodyOffset = offset;
}
