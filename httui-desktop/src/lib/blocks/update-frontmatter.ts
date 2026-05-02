// V2 / cenário 4.5 — mutators that rewrite YAML frontmatter inside a
// markdown document. Pure string-in / string-out: each function takes
// the full document content and the new field value, and returns the
// modified document. Mirrors the Rust slice-1 schema (single-line scalar
// `title:` / `abstract:`, flow-style `tags: [a, b]`).
//
// These helpers are deliberately conservative — they only modify the
// minimum slice of the document needed and preserve everything else
// (body, leading whitespace, other frontmatter keys, line-ending
// style). The DocHeader's editable fields call into these on commit so
// the source of truth stays the `.md` file, not React state.

interface SplitDoc {
  before: string;
  fmBody: string;
  after: string;
}

/**
 * Split the document into `{ before, fmBody, after }` when it has a
 * frontmatter block, else `null`. `before` is `"---\n"` (or `"---\r\n"`),
 * `fmBody` is the YAML between fences (joined including its trailing
 * newlines), `after` is `"---\n"` followed by the rest of the document.
 *
 * Used by the field mutators below to operate on `fmBody` independently
 * of body text. The opening and closing fences are kept as-is in the
 * output so the line-ending style is preserved.
 */
function splitDoc(content: string): SplitDoc | null {
  const stripped = content.startsWith("\u{FEFF}") ? content.slice(1) : content;
  let openFenceLen: number;
  if (stripped.startsWith("---\n")) {
    openFenceLen = 4;
  } else if (stripped.startsWith("---\r\n")) {
    openFenceLen = 5;
  } else {
    return null;
  }

  const before = stripped.slice(0, openFenceLen);
  let rest = stripped.slice(openFenceLen);
  const buf: string[] = [];
  while (rest.length > 0) {
    const lineEndAbs = rest.indexOf("\n");
    const lineEnd = lineEndAbs < 0 ? rest.length : lineEndAbs + 1;
    const line = rest.slice(0, lineEnd);
    const trimmedLine = line.replace(/[\r\n]+$/, "");
    if (trimmedLine === "---") {
      // Closer found. `line` IS the close-fence including its newline
      // (if present); push everything after it as `after`.
      const after = line + rest.slice(lineEnd);
      return { before, fmBody: buf.join(""), after };
    }
    buf.push(line);
    rest = rest.slice(lineEnd);
  }
  // Hit EOF without seeing the closing fence — treat as no frontmatter.
  return null;
}

/**
 * Locate the first top-level `title:` line in a YAML body. Returns the
 * line's index range within `fmBody`, or `null` when the key is absent.
 * Indented lines are ignored (the Rust slice-1 parser ditto).
 */
function findFieldLineRange(
  fmBody: string,
  field: string,
): { from: number; to: number; line: string } | null {
  const lines = fmBody.split("\n");
  let cursor = 0;
  for (const line of lines) {
    const lineLen = line.length;
    const lineEndCursor = cursor + lineLen;
    // skip leading whitespace check — only top-level keys count.
    if (line.length > 0 && (line[0] === " " || line[0] === "\t")) {
      cursor = lineEndCursor + 1;
      continue;
    }
    const trimmed = line.replace(/[\r\n]+$/, "");
    const colonIdx = trimmed.indexOf(":");
    if (colonIdx > 0) {
      const key = trimmed.slice(0, colonIdx).trim();
      if (key === field) {
        return { from: cursor, to: lineEndCursor, line };
      }
    }
    cursor = lineEndCursor + 1;
  }
  return null;
}

/**
 * Encode a scalar value for a single-line YAML field. Quotes the value
 * with double quotes when it contains characters YAML would parse
 * specially (`:`, `#`, leading `-`/`?`/`!`/`*`/`&`, trailing whitespace,
 * leading whitespace, or empty). Otherwise emits as plain.
 */
function encodeScalar(value: string): string {
  if (value === "") return '""';
  const needsQuoting =
    /[:#"\\\n]/.test(value) ||
    /^[-?!&*]/.test(value) ||
    /^\s|\s$/.test(value) ||
    value === "~" ||
    value === "null" ||
    value === "true" ||
    value === "false" ||
    /^-?\d/.test(value);
  if (!needsQuoting) return value;
  return `"${value.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
}

/**
 * Replace, insert, or prepend a `title:` field in a markdown document's
 * YAML frontmatter and return the rewritten document.
 *
 * Cases:
 *   1. `newTitle.trim()` is empty → return `content` unchanged. The
 *      virtual-mode contract is that the file on disk doesn't gain a
 *      frontmatter until the user types a real title.
 *   2. Frontmatter exists with a `title:` line → replace that line.
 *   3. Frontmatter exists without `title:` → insert `title: X` as the
 *      first line of the YAML body.
 *   4. No frontmatter → prepend `---\ntitle: X\n---\n\n` to the doc.
 */
export function updateFrontmatterTitle(
  content: string,
  newTitle: string,
): string {
  const trimmed = newTitle.trim();
  if (trimmed.length === 0) return content;

  const encoded = encodeScalar(trimmed);
  const titleLine = `title: ${encoded}`;

  const split = splitDoc(content);
  if (!split) {
    // Case 4 — no frontmatter. Prepend a fresh block. Preserve a
    // separator blank line so the rest of the doc stays readable.
    return `---\n${titleLine}\n---\n\n${content}`;
  }

  const range = findFieldLineRange(split.fmBody, "title");
  if (range) {
    // Case 2 — splice the existing title line.
    const next =
      split.fmBody.slice(0, range.from) +
      titleLine +
      split.fmBody.slice(range.to);
    return split.before + next + split.after;
  }

  // Case 3 — insert as the first body line. Preserve any existing
  // content by appending it after our new line.
  const insertion =
    split.fmBody.length === 0
      ? `${titleLine}\n`
      : `${titleLine}\n${split.fmBody}`;
  return split.before + insertion + split.after;
}
