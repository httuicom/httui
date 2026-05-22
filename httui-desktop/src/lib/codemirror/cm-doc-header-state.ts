// DocHeader portal registry + per-entry state.
//
// Extracted from `cm-doc-header.tsx` so the CM6 widget / extension
// factory file stays focused on CM6 plumbing (StateField, decorations,
// keymap, transactionFilter). This sibling owns:
//
//  - the cross-instance entries map + subscription primitives that
//    `DocHeaderWidgetPortal.tsx` reads via useSyncExternalStore,
//  - the live frontmatter parse cache + executable-block count synced
//    on every doc change,
//  - the title-input registration + body-focus return,
//  - the `dispatchDocReplace` helper used by the editable callbacks.
//
// The CM6 extension factory imports from here and wires the StateField
// to call `syncEntryFrontmatter` on every doc change.
import {
  Annotation,
  EditorSelection,
  type StateField,
} from "@codemirror/state";
import { EditorView, type DecorationSet } from "@codemirror/view";

import { extractFrontmatter } from "@/lib/blocks/extract-frontmatter-tags";
import type { DocHeaderFrontmatter } from "@/components/layout/docheader/docheader-derive";

export interface FrontmatterRange {
  /** Inclusive start offset (always 0 — frontmatter must be at top). */
  from: number;
  /** Exclusive end offset, including the trailing newline of the closing
   * fence. Use this directly as the upper bound of `Decoration.replace`. */
  to: number;
}

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
  /** Number of executable fenced blocks in the body — tracked alongside
   *  the frontmatter so the meta-strip "N blocks" chip stays live without
   *  the portal needing to scan content itself. */
  blockCount: number;
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
    blockCount: 0,
  };
  entries.set(id, next);
  return next;
}

export function registerContainer(
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
      blockCount: 0,
    });
  }
  notify();
}

export function unregisterContainer(id: string) {
  if (!entries.has(id)) return;
  entries.delete(id);
  notify();
}

/** Bind the CM6 view that owns this DocHeader. The widget calls this
 * during `toDOM` so React consumers can dispatch transactions back into
 * the editor. Idempotent — re-binding the same view is a no-op. */
export function bindView(id: string, view: EditorView, container: HTMLElement) {
  const entry = ensureEntry(id, container);
  if (entry.view !== view) {
    entry.view = view;
  }
}

function frontmatterEqual(
  a: DocHeaderFrontmatter | null,
  b: DocHeaderFrontmatter | null,
): boolean {
  if (a === b) return true;
  if (!a || !b) return false;
  if (a.title !== b.title) return false;
  if (a.abstract !== b.abstract) return false;
  if (a.error !== b.error) return false;
  const aTags = a.tags ?? [];
  const bTags = b.tags ?? [];
  if (aTags.length !== bTags.length) return false;
  for (let i = 0; i < aTags.length; i++)
    if (aTags[i] !== bTags[i]) return false;
  const aTasks = a.tasks ?? [];
  const bTasks = b.tasks ?? [];
  if (aTasks.length !== bTasks.length) return false;
  for (let i = 0; i < aTasks.length; i++) {
    if (aTasks[i].text !== bTasks[i].text) return false;
    if (aTasks[i].done !== bTasks[i].done) return false;
  }
  return true;
}

function frontmatterFromDoc(doc: string): DocHeaderFrontmatter | null {
  const fm = extractFrontmatter(doc);
  if (
    fm.title === undefined &&
    fm.abstract === undefined &&
    fm.tags.length === 0 &&
    fm.tasks.length === 0 &&
    fm.error === undefined
  ) {
    return null;
  }
  return {
    title: fm.title,
    abstract: fm.abstract,
    tags: fm.tags,
    tasks: fm.tasks,
    error: fm.error,
  };
}

/** Count executable fenced blocks in the doc (`http`, `db-*`). Used
 *  to keep the meta-strip "N blocks" chip live without forcing the
 *  React portal to scan content itself. */
function countExecutableBlocks(doc: string): number {
  let count = 0;
  let inFence = false;
  for (const line of doc.split("\n")) {
    if (inFence) {
      if (line.startsWith("```")) inFence = false;
      continue;
    }
    if (
      line.startsWith("```http") ||
      line.startsWith("```db-") ||
      line === "```http" ||
      line === "```db"
    ) {
      count += 1;
      inFence = true;
    } else if (line.startsWith("```")) {
      inFence = true;
    }
  }
  return count;
}

/** Set the live frontmatter + block count for an entry and notify
 *  subscribers if either changed. Called from the StateField on every
 *  doc change — equality gating keeps the React portal from re-
 *  rendering on body keystrokes that didn't touch the frontmatter
 *  fields or the block layout. */
export function syncEntryFrontmatter(id: string, doc: string) {
  const entry = entries.get(id);
  if (!entry) return;
  const nextFm = frontmatterFromDoc(doc);
  const nextBlocks = countExecutableBlocks(doc);
  const fmChanged = !frontmatterEqual(entry.frontmatter, nextFm);
  const blocksChanged = entry.blockCount !== nextBlocks;
  if (!fmChanged && !blocksChanged) return;
  entry.frontmatter = nextFm;
  entry.blockCount = nextBlocks;
  notify();
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

const fieldByInstance = new Map<
  string,
  StateField<{ decorations: DecorationSet; range: FrontmatterRange | null }>
>();

export function setFieldForInstance(
  id: string,
  field: StateField<{
    decorations: DecorationSet;
    range: FrontmatterRange | null;
  }>,
) {
  fieldByInstance.set(id, field);
}

export function getFieldFor(id: string) {
  return fieldByInstance.get(id) as StateField<{
    decorations: DecorationSet;
    range: FrontmatterRange | null;
  }>;
}

/**
 * Annotation tag set on transactions that intentionally rewrite the
 * frontmatter (title / abstract / tags / checklist callbacks). The
 * transactionFilter checks for this tag to allow the rewrite through;
 * unannotated changes that touch the hidden frontmatter range get
 * blocked — that's how Backspace / Delete / cut on the first body
 * line stays safe, otherwise the user's hidden YAML would silently
 * disappear.
 */
export const FRONTMATTER_REWRITE = Annotation.define<true>();

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
    annotations: FRONTMATTER_REWRITE.of(true),
  });
}
