// Numbered-section-heading decoration.
//
// Renders a leading "1.", "2.", ... before each top-level `#` /
// `##` heading line. Numbering is positional across the whole
// document and rebuilds on every state change, so insert / delete
// re-numbers automatically. Headings inside fenced code blocks
// (```...``` or `~~~...~~~`) are skipped — they're prose-rendered
// as code, not document outline.
//
// The decoration is a `Decoration.line` that adds a
// `cm-numbered-heading` class plus `data-heading-number` and
// `data-heading-level` (1 or 2) attributes — actual styling (accent
// circle, serif font, H1 size) lives in the editor theme so this
// module stays presentation-free.

import { RangeSetBuilder, StateField, type Extension } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView } from "@codemirror/view";

const HEADING_RE = /^(#{1,2})\s+\S/;
const FENCE_RE = /^(```|~~~)/;

interface BuildResult {
  decorations: DecorationSet;
  /** Total numbered-heading lines found — exposed for tests. */
  count: number;
}

/** Walk the document and build a `RangeSet` of `cm-numbered-heading`
 * line decorations. Pure function: takes a doc, returns the
 * decoration set. Easy to unit-test. */
export function buildHeadingDecorations(doc: {
  lines: number;
  line: (n: number) => { from: number; text: string };
}): BuildResult {
  const builder = new RangeSetBuilder<Decoration>();
  let inFence = false;
  let fenceMarker: string | null = null;
  let counter = 0;

  for (let i = 1; i <= doc.lines; i += 1) {
    const line = doc.line(i);
    const text = line.text;

    // Toggle fence state. A fence line starts with ``` or ~~~. The
    // closing fence must use the same marker; any non-matching
    // marker is treated as content inside the fence.
    const fenceMatch = FENCE_RE.exec(text);
    if (fenceMatch) {
      const marker = fenceMatch[1];
      if (!inFence) {
        inFence = true;
        fenceMarker = marker;
      } else if (marker === fenceMarker) {
        inFence = false;
        fenceMarker = null;
      }
      continue;
    }

    if (inFence) continue;

    const headingMatch = HEADING_RE.exec(text);
    if (!headingMatch) continue;

    counter += 1;
    const level = headingMatch[1].length; // 1 for `#`, 2 for `##`
    builder.add(
      line.from,
      line.from,
      Decoration.line({
        class: "cm-numbered-heading",
        attributes: {
          "data-heading-number": String(counter),
          "data-heading-level": String(level),
        },
      }),
    );
  }

  return { decorations: builder.finish(), count: counter };
}

const numberedHeadingsField = StateField.define<DecorationSet>({
  create(state) {
    return buildHeadingDecorations(state.doc).decorations;
  },
  update(value, tr) {
    if (!tr.docChanged) return value;
    return buildHeadingDecorations(tr.state.doc).decorations;
  },
  provide: (f) => EditorView.decorations.from(f),
});

export function numberedHeadings(): Extension {
  return numberedHeadingsField;
}
