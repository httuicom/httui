// Outline extractor.
//
// Pulls H1/H2/H3 markdown headings out of a runbook so the
// right-sidebar Outline tab can render a click-to-scroll table of
// contents. Pure string-in / array-out — the CM6-side hook adapts
// state.doc into the same input.
//
// Mirrors the fenced-code guard in `cm-numbered-headings.tsx`:
// headings inside ```...``` or ~~~...~~~ blocks are body content
// (HTTP/SQL/etc.) not document outline. YAML frontmatter (`---`
// fenced at offset 0) is skipped too — keys like `title:` are
// metadata, not headings.
//
// `# tag` (no space, no body) doesn't match the heading regex
// because we require at least one non-whitespace character after
// the run of `#`s — keeps `#payments` body tags out of the outline.

const HEADING_RE = /^(#{1,6})\s+(\S.*)$/;
const FENCE_RE = /^(```|~~~)/;

export interface OutlineEntry {
  /** Heading level — 1 for H1, 2 for H2, etc. */
  level: number;
  /** Heading text with trailing `#` markers and whitespace trimmed. */
  text: string;
  /** 1-indexed line number; matches CM6's `doc.line(n)` convention. */
  line: number;
  /** Byte offset of the line start; ready for `EditorView.dispatch
   *  ({ selection: { anchor: offset } })`. */
  offset: number;
}

export interface ExtractOutlineOptions {
  /** Cap on heading levels emitted. Default `3` per spec
   *  ("H1/H2/H3"). Pass `6` to surface every level. */
  maxLevel?: number;
}

/** Walk `content` and return one [`OutlineEntry`] per markdown
 *  heading at or below `maxLevel`. Skips:
 *  - leading YAML frontmatter (between `---` fences at offset 0)
 *  - any heading inside a fenced code block
 *  - lines that don't have a body after the `#` run (e.g. `#`
 *    alone, or `#tag` body-tag patterns) */
export function extractOutline(
  content: string,
  options: ExtractOutlineOptions = {},
): OutlineEntry[] {
  const maxLevel = options.maxLevel ?? 3;
  if (!content) return [];

  // Frontmatter: walk a fence at offset 0; the closing `---` line
  // ends the front matter. Any text after that fence is body.
  let bodyStartOffset = 0;
  let bodyStartLine = 1;
  if (content.startsWith("---\n") || content.startsWith("---\r\n")) {
    const eol1 = content.indexOf("\n");
    let scanFrom = eol1 + 1;
    let scanLine = 2;
    while (scanFrom < content.length) {
      const eol = content.indexOf("\n", scanFrom);
      const lineEnd = eol === -1 ? content.length : eol;
      const lineText = content.slice(scanFrom, lineEnd).replace(/\r$/, "");
      if (lineText === "---") {
        bodyStartOffset = lineEnd + 1;
        bodyStartLine = scanLine + 1;
        break;
      }
      scanFrom = lineEnd + 1;
      scanLine += 1;
    }
  }

  const entries: OutlineEntry[] = [];
  let inFence = false;
  let fenceMarker: string | null = null;
  let offset = bodyStartOffset;
  let lineNo = bodyStartLine;

  while (offset < content.length) {
    const eol = content.indexOf("\n", offset);
    const lineEnd = eol === -1 ? content.length : eol;
    const rawLine = content.slice(offset, lineEnd);
    const text = rawLine.replace(/\r$/, "");

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
    } else if (!inFence) {
      const m = HEADING_RE.exec(text);
      if (m) {
        const level = m[1].length;
        if (level <= maxLevel) {
          // Strip a closing-`#` run + surrounding whitespace
          // (atx-style closed headings: `## Foo ##`).
          const headingText = m[2].replace(/\s+#+\s*$/, "").trim();
          entries.push({
            level,
            text: headingText,
            line: lineNo,
            offset,
          });
        }
      }
    }

    if (eol === -1) break;
    offset = eol + 1;
    lineNo += 1;
  }

  return entries;
}
