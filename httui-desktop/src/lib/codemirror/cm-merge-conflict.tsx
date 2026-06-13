// V10 follow-up — git merge-conflict awareness in the markdown editor.
//
// Surfaced by manual testing of opening a conflicted `.md`
// rendered the `<<<<<<< / ======= / >>>>>>>` markers as plain
// markdown, so the user couldn't tell the file was in conflict (and
// auto-save could regrave it). This extension decorates each conflict
// hunk — colored "ours"/"theirs" line backgrounds, highlighted marker
// lines — and adds an inline toolbar to accept one side (or both),
// dispatching the doc edit. Resolution still goes through the git
// panel for the final `git add`; this just makes the conflict
// unmistakable and one-click resolvable in-place.

import {
  Decoration,
  type DecorationSet,
  EditorView,
  WidgetType,
} from "@codemirror/view";
import {
  RangeSetBuilder,
  StateField,
  type Extension,
  type Text,
} from "@codemirror/state";

const OURS_RE = /^<{7} /;
const SEP_RE = /^={7}\s*$/;
const THEIRS_RE = /^>{7} /;

export interface ConflictRegion {
  /** 1-indexed line of the `<<<<<<<` marker. */
  oursMarker: number;
  /** 1-indexed line of the `=======` separator. */
  separator: number;
  /** 1-indexed line of the `>>>>>>>` marker. */
  theirsMarker: number;
}

/** Scan the document for well-formed conflict hunks. A hunk needs
 *  `<<<<<<<` then `=======` then `>>>>>>>` in order; malformed/partial
 *  marker runs are ignored so a half-typed marker doesn't decorate. */
export function parseConflictRegions(doc: Text): ConflictRegion[] {
  const regions: ConflictRegion[] = [];
  let ours = -1;
  let sep = -1;
  for (let i = 1; i <= doc.lines; i++) {
    const text = doc.line(i).text;
    if (OURS_RE.test(text)) {
      ours = i;
      sep = -1;
    } else if (SEP_RE.test(text) && ours !== -1) {
      sep = i;
    } else if (THEIRS_RE.test(text) && ours !== -1 && sep !== -1) {
      regions.push({ oursMarker: ours, separator: sep, theirsMarker: i });
      ours = -1;
      sep = -1;
    }
  }
  return regions;
}

type Side = "ours" | "theirs" | "both";

/** Replace the whole hunk (markers included) with the chosen side. */
function resolveRegion(
  view: EditorView,
  region: ConflictRegion,
  side: Side,
): void {
  const doc = view.state.doc;
  const from = doc.line(region.oursMarker).from;
  const to = doc.line(region.theirsMarker).to;
  const slice = (a: number, b: number): string => {
    if (a > b) return "";
    const lines: string[] = [];
    for (let i = a; i <= b; i++) lines.push(doc.line(i).text);
    return lines.join("\n");
  };
  const ours = slice(region.oursMarker + 1, region.separator - 1);
  const theirs = slice(region.separator + 1, region.theirsMarker - 1);
  const insert =
    side === "ours" ? ours : side === "theirs" ? theirs : `${ours}\n${theirs}`;
  view.dispatch({ changes: { from, to, insert } });
}

class ConflictToolbarWidget extends WidgetType {
  constructor(readonly line: number) {
    super();
  }

  eq(other: ConflictToolbarWidget): boolean {
    return other.line === this.line;
  }

  toDOM(view: EditorView): HTMLElement {
    const bar = document.createElement("span");
    bar.className = "cm-conflict-toolbar";
    bar.contentEditable = "false";

    const mk = (text: string, side: Side) => {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "cm-conflict-btn";
      btn.dataset.side = side;
      btn.textContent = text;
      btn.addEventListener("mousedown", (e) => {
        // mousedown (not click) so CM6 doesn't move the selection
        // into the about-to-be-removed range first.
        e.preventDefault();
        e.stopPropagation();
        const pos = view.posAtDOM(bar);
        const lineNo = view.state.doc.lineAt(pos).number;
        const region = parseConflictRegions(view.state.doc).find(
          (r) => r.oursMarker === lineNo,
        );
        if (region) resolveRegion(view, region, side);
      });
      return btn;
    };

    bar.appendChild(mk("Accept current", "ours"));
    bar.appendChild(mk("Accept incoming", "theirs"));
    bar.appendChild(mk("Accept both", "both"));
    return bar;
  }

  ignoreEvent(): boolean {
    return false;
  }
}

const oursMarkerLine = Decoration.line({
  class: "cm-conflict-line cm-conflict-marker cm-conflict-ours-marker",
});
const oursLine = Decoration.line({
  class: "cm-conflict-line cm-conflict-ours",
});
const sepLine = Decoration.line({
  class: "cm-conflict-line cm-conflict-sep",
});
const theirsLine = Decoration.line({
  class: "cm-conflict-line cm-conflict-theirs",
});
const theirsMarkerLine = Decoration.line({
  class: "cm-conflict-line cm-conflict-marker cm-conflict-theirs-marker",
});

function buildDecorations(doc: Text): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>();
  for (const region of parseConflictRegions(doc)) {
    for (let i = region.oursMarker; i <= region.theirsMarker; i++) {
      const line = doc.line(i);
      let deco: Decoration;
      if (i === region.oursMarker) deco = oursMarkerLine;
      else if (i < region.separator) deco = oursLine;
      else if (i === region.separator) deco = sepLine;
      else if (i < region.theirsMarker) deco = theirsLine;
      else deco = theirsMarkerLine;
      builder.add(line.from, line.from, deco);
      if (i === region.oursMarker) {
        // Inline accept actions at the end of the `<<<<<<<` line —
        // VS Code-style, not a full-width block bar.
        builder.add(
          line.to,
          line.to,
          Decoration.widget({
            widget: new ConflictToolbarWidget(region.oursMarker),
            side: 1,
          }),
        );
      }
    }
  }
  return builder.finish();
}

// Block decorations (the inline accept toolbar) can't be emitted
// from a ViewPlugin — CM6 requires a StateField for those.
const conflictField = StateField.define<DecorationSet>({
  create(state) {
    return buildDecorations(state.doc);
  },
  update(deco, tr) {
    if (!tr.docChanged) return deco;
    // A conflict region only exists once a `<<<<<<<` marker line does,
    // so the only edits that can create one insert a `<`. When the doc
    // has no conflict and this edit inserts none, map the (empty)
    // decorations forward instead of rescanning every line — the common
    // case in a large document with no merge in progress.
    const mapped = deco.map(tr.changes);
    if (mapped.size > 0) return buildDecorations(tr.state.doc);
    let mightStartConflict = false;
    tr.changes.iterChanges((_fromA, _toA, _fromB, _toB, inserted) => {
      if (!mightStartConflict && inserted.toString().includes("<"))
        mightStartConflict = true;
    });
    return mightStartConflict ? buildDecorations(tr.state.doc) : mapped;
  },
  provide: (f) => EditorView.decorations.from(f),
});

const conflictTheme = EditorView.theme({
  ".cm-conflict-line": { fontFamily: "var(--chakra-fonts-mono)" },
  ".cm-conflict-ours": {
    backgroundColor: "rgba(220, 38, 38, 0.10)",
  },
  ".cm-conflict-theirs": {
    backgroundColor: "rgba(22, 163, 74, 0.10)",
  },
  ".cm-conflict-sep": {
    backgroundColor: "rgba(120, 120, 120, 0.15)",
  },
  ".cm-conflict-marker": {
    backgroundColor: "rgba(120, 120, 120, 0.22)",
    fontWeight: "700",
  },
  ".cm-conflict-toolbar": {
    display: "inline-flex",
    alignItems: "center",
    gap: "4px",
    marginLeft: "10px",
    verticalAlign: "middle",
    userSelect: "none",
  },
  ".cm-conflict-btn": {
    cursor: "pointer",
    border: "1px solid rgba(120,120,120,0.45)",
    borderRadius: "3px",
    background: "rgba(120,120,120,0.12)",
    color: "var(--chakra-colors-fg-muted, #aaa)",
    fontFamily: "var(--chakra-fonts-mono)",
    fontSize: "9px",
    lineHeight: "1.2",
    padding: "0 5px",
  },
  ".cm-conflict-btn:hover": {
    backgroundColor: "rgba(120,120,120,0.3)",
    color: "var(--chakra-colors-fg, #eee)",
  },
});

/** Markdown-editor extension: decorate git conflict hunks + inline
 *  accept actions. Inserted after `hybridRendering()` so its line
 *  backgrounds layer over the live-preview styling. */
export function mergeConflict(): Extension {
  return [conflictField, conflictTheme];
}
