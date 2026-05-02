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
// one DocHeader). React mounts via `DocHeaderWidgetPortal.tsx`.

import {
  EditorSelection,
  Prec,
  RangeSetBuilder,
  StateField,
  type EditorState,
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

import { extractFrontmatter } from "@/lib/blocks/extract-frontmatter-tags";
import type { DocHeaderFrontmatter } from "@/components/layout/docheader/docheader-derive";

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

export interface FrontmatterRange {
  /** Inclusive start offset (always 0 — frontmatter must be at top). */
  from: number;
  /** Exclusive end offset, including the trailing newline of the closing
   * fence. Use this directly as the upper bound of `Decoration.replace`. */
  to: number;
}

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

// ───── Portal registry ─────

export interface DocHeaderEntry {
  id: string;
  container: HTMLElement;
  /** Whether the host doc currently has a frontmatter block. The portal
   *  uses this to know whether to render virtual-mode placeholders. */
  hasFrontmatter: boolean;
  /** The CM6 view that owns this DocHeader. Set when the widget mounts;
   *  the title input uses it to dispatch a back-to-body selection on
   *  Enter / ArrowDown / Escape. */
  view: EditorView | null;
  /** Title input element. Registered by `DocHeaderTitleInput`; CM6
   *  ArrowUp at body start focuses it. */
  titleInput: HTMLInputElement | null;
  /** Last cursor offset inside the body (after the frontmatter range).
   *  Tracked by an `EditorView.updateListener` so leaving + re-entering
   *  the body restores the previous position. Defaults to body start
   *  when the user has never placed a cursor in the body. */
  lastBodyOffset: number;
  /** Live frontmatter parsed from the doc by the `EditorView`'s
   *  StateField. The portal reads this directly so the React tree
   *  always sees the same shape CM6 sees — bypasses the non-reactive
   *  `editorContents` Map in the pane store. `null` mirrors the legacy
   *  "no frontmatter" sentinel from `DocHeaderedEditor`. */
  frontmatter: DocHeaderFrontmatter | null;
}

const entries = new Map<string, DocHeaderEntry>();
const listeners = new Set<() => void>();
let portalVersion = 0;

function notify() {
  portalVersion++;
  for (const fn of listeners) fn();
}

export function subscribeToDocHeaderPortals(cb: () => void): () => void {
  listeners.add(cb);
  return () => {
    listeners.delete(cb);
  };
}

export function getDocHeaderPortalVersion(): number {
  return portalVersion;
}

export function getDocHeaderEntries(): ReadonlyMap<string, DocHeaderEntry> {
  return entries;
}

function ensureEntry(id: string, container: HTMLElement): DocHeaderEntry {
  const prev = entries.get(id);
  if (prev) return prev;
  const next: DocHeaderEntry = {
    id,
    container,
    hasFrontmatter: false,
    view: null,
    titleInput: null,
    lastBodyOffset: 0,
    frontmatter: null,
  };
  entries.set(id, next);
  return next;
}

function registerContainer(
  id: string,
  container: HTMLElement,
  hasFrontmatter: boolean,
) {
  const prev = entries.get(id);
  if (
    prev &&
    prev.container === container &&
    prev.hasFrontmatter === hasFrontmatter
  ) {
    return;
  }
  if (prev) {
    prev.container = container;
    prev.hasFrontmatter = hasFrontmatter;
  } else {
    entries.set(id, {
      id,
      container,
      hasFrontmatter,
      view: null,
      titleInput: null,
      lastBodyOffset: 0,
      frontmatter: null,
    });
  }
  notify();
}

function frontmatterEqual(
  a: DocHeaderFrontmatter | null,
  b: DocHeaderFrontmatter | null,
): boolean {
  if (a === b) return true;
  if (!a || !b) return false;
  if (a.title !== b.title) return false;
  if (a.abstract !== b.abstract) return false;
  const aTags = a.tags ?? [];
  const bTags = b.tags ?? [];
  if (aTags.length !== bTags.length) return false;
  for (let i = 0; i < aTags.length; i++) if (aTags[i] !== bTags[i]) return false;
  const aPre = a.preflight ?? [];
  const bPre = b.preflight ?? [];
  if (aPre.length !== bPre.length) return false;
  for (let i = 0; i < aPre.length; i++) {
    if (aPre[i].text !== bPre[i].text) return false;
    if (aPre[i].done !== bPre[i].done) return false;
  }
  return true;
}

function frontmatterFromDoc(doc: string): DocHeaderFrontmatter | null {
  const fm = extractFrontmatter(doc);
  if (
    fm.title === undefined &&
    fm.abstract === undefined &&
    fm.tags.length === 0 &&
    fm.preflight.length === 0
  ) {
    return null;
  }
  return {
    title: fm.title,
    abstract: fm.abstract,
    tags: fm.tags,
    preflight: fm.preflight,
  };
}

/** Set the live frontmatter for an entry and notify subscribers if the
 *  parsed shape actually changed. Called from the StateField on every
 *  doc change — gating on equality keeps the React portal from
 *  re-rendering on body keystrokes that didn't touch the frontmatter. */
function syncEntryFrontmatter(id: string, doc: string) {
  const entry = entries.get(id);
  if (!entry) return;
  const next = frontmatterFromDoc(doc);
  if (frontmatterEqual(entry.frontmatter, next)) return;
  entry.frontmatter = next;
  notify();
}

/**
 * Replace the editor's doc with `newContent`, dispatching a minimal
 * change record (computed from the longest common prefix / suffix) so
 * CM6 maps the user's body cursor automatically. Used by header
 * callbacks (title / abstract / tags / checklist) to round-trip a
 * frontmatter rewrite through CM6's update cycle — that fires the
 * editor's onChange (which writes to the pane store's content map)
 * AND triggers the StateField below (which re-parses the frontmatter
 * and refreshes the React portal).
 */
export function dispatchDocReplace(view: EditorView, newContent: string) {
  const oldContent = view.state.doc.toString();
  if (oldContent === newContent) return;

  let prefix = 0;
  const minLen = Math.min(oldContent.length, newContent.length);
  while (prefix < minLen && oldContent[prefix] === newContent[prefix]) prefix++;

  let suffix = 0;
  const maxSuffix = minLen - prefix;
  while (
    suffix < maxSuffix &&
    oldContent[oldContent.length - 1 - suffix] ===
      newContent[newContent.length - 1 - suffix]
  ) {
    suffix++;
  }

  view.dispatch({
    changes: {
      from: prefix,
      to: oldContent.length - suffix,
      insert: newContent.slice(prefix, newContent.length - suffix),
    },
  });
}

function unregisterContainer(id: string) {
  if (!entries.has(id)) return;
  entries.delete(id);
  notify();
}

/** Bind the CM6 view that owns this DocHeader. The widget calls this
 * during `toDOM` so React consumers can dispatch transactions back into
 * the editor. Idempotent — re-binding the same view is a no-op. */
function bindView(id: string, view: EditorView, container: HTMLElement) {
  const entry = ensureEntry(id, container);
  if (entry.view !== view) {
    entry.view = view;
  }
}

/** Register the DocHeader's title input element. Called by
 * `DocHeaderTitleInput` on mount; the CM6 keymap reads this to focus
 * the input on `ArrowUp` at body start. */
export function registerDocHeaderTitleInput(
  id: string,
  input: HTMLInputElement | null,
) {
  const entry = entries.get(id);
  if (!entry) return;
  entry.titleInput = input;
}

/** Move the editor selection to the last body offset (or body start
 * when the user hasn't placed a cursor in the body yet) and focus the
 * view. Used by the title input to leave focus on Enter / ArrowDown /
 * Escape. */
export function returnFocusToBody(id: string) {
  const entry = entries.get(id);
  if (!entry || !entry.view) return;
  const view = entry.view;
  const value = view.state.field(getFieldFor(id), false);
  const bodyStart = value?.range?.to ?? 0;
  const target =
    entry.lastBodyOffset >= bodyStart ? entry.lastBodyOffset : bodyStart;
  const clamped = Math.min(Math.max(target, bodyStart), view.state.doc.length);
  view.dispatch({
    selection: EditorSelection.cursor(clamped),
    scrollIntoView: true,
  });
  view.focus();
}

// Per-extension state field reference — the keymap and the
// `returnFocusToBody` helper need it to read the frontmatter range out
// of the live editor state.
const fieldByInstance = new Map<
  string,
  StateField<{ decorations: DecorationSet; range: FrontmatterRange | null }>
>();

function getFieldFor(id: string) {
  return fieldByInstance.get(id) as StateField<{
    decorations: DecorationSet;
    range: FrontmatterRange | null;
  }>;
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

  fieldByInstance.set(instanceId, field);

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
    const entry = entries.get(instanceId);
    if (entry) entry.lastBodyOffset = head;
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
        const entry = entries.get(instanceId);
        const input = entry?.titleInput;
        if (!input) return false;
        // Remember where the cursor was so the input's
        // back-to-body action can restore it.
        if (entry) entry.lastBodyOffset = sel.head;
        input.focus();
        // Most browsers preserve the caret on a refocus. Move it to
        // the end of the value for predictable Notion-like UX.
        const len = input.value.length;
        input.setSelectionRange(len, len);
        return true;
      },
    },
  ];

  return {
    extension: [field, updateListener, Prec.high(keymap.of(navKeymap))],
    instanceId,
  };
}
