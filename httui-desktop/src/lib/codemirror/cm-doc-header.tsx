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
  type DecorationSet,
} from "@codemirror/view";

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
  entries.set(id, { id, container, hasFrontmatter });
  notify();
}

function unregisterContainer(id: string) {
  if (!entries.has(id)) return;
  entries.delete(id);
  notify();
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

  toDOM(): HTMLElement {
    const div = document.createElement("div");
    div.className = "cm-doc-header-portal";
    div.setAttribute("data-doc-header-id", this.instanceId);
    div.contentEditable = "false";
    registerContainer(this.instanceId, div, this.hasFrontmatter);
    return div;
  }

  updateDOM(dom: HTMLElement): boolean {
    registerContainer(this.instanceId, dom, this.hasFrontmatter);
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
    // Let React handle clicks / keys inside the widget DOM.
    return false;
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

  return { extension: [field], instanceId };
}
