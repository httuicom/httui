// mutators that rewrite YAML frontmatter inside a markdown document.
// Pure string-in / string-out: each function takes
// the full document content and the new field value, and returns the
// modified document. Mirrors the Rust slice-1 schema (single-line scalar
// `title:` / `abstract:`, flow-style `tags: [a, b]`,
// `tasks: ["[ ] foo", "[x] bar"]`).
//
// These helpers are deliberately conservative — they only modify the
// minimum slice of the document needed and preserve everything else
// (body, leading whitespace, other frontmatter keys, line-ending
// style). The DocHeader's editable fields call into these on commit so
// the source of truth stays the `.md` file, not React state.

import { stringifyTaskItem, type TaskItem } from "./task-item";

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

type InsertAt = "start" | "end";

/**
 * Generic single-line scalar replacement / insertion. Drives both
 * `updateFrontmatterTitle` and `updateFrontmatterAbstract` with the
 * only behavioural difference being where the new line lands when the
 * field is missing (`title` at the top, `abstract` at the bottom).
 */
function updateFrontmatterScalar(
  content: string,
  field: string,
  rawValue: string,
  opts: { insertAt: InsertAt },
): string {
  const trimmed = rawValue.trim();
  if (trimmed.length === 0) return content;

  const encoded = encodeScalar(trimmed);
  const newLine = `${field}: ${encoded}`;

  const split = splitDoc(content);
  if (!split) {
    // No frontmatter — prepend a fresh block. Preserve a separator
    // blank line so the rest of the doc stays readable.
    return `---\n${newLine}\n---\n\n${content}`;
  }

  const range = findFieldLineRange(split.fmBody, field);
  if (range) {
    const next =
      split.fmBody.slice(0, range.from) +
      newLine +
      split.fmBody.slice(range.to);
    return split.before + next + split.after;
  }

  // Field is missing — insert at the requested side. `fmBody` always
  // ends in `\n` when non-empty (each YAML line carries its own
  // terminator), so concatenation produces well-formed output.
  let nextBody: string;
  if (split.fmBody.length === 0) {
    nextBody = `${newLine}\n`;
  } else if (opts.insertAt === "start") {
    nextBody = `${newLine}\n${split.fmBody}`;
  } else {
    nextBody = `${split.fmBody}${newLine}\n`;
  }
  return split.before + nextBody + split.after;
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
  return updateFrontmatterScalar(content, "title", newTitle, {
    insertAt: "start",
  });
}

/**
 * Replace, insert, or prepend an `abstract:` field. Newlines in the
 * input are collapsed to a single space (the slice-1 schema only
 * supports single-line scalars; multi-line block scalars are deferred).
 * When the field is missing, the new line lands at the END of the
 * frontmatter body so it sits below the title.
 */
export function updateFrontmatterAbstract(
  content: string,
  newAbstract: string,
): string {
  // Single-line collapse. We trim duplicated whitespace as well so
  // pasted multi-paragraph text writes back as a tidy one-liner.
  const oneLine = newAbstract.replace(/\s+/g, " ");
  return updateFrontmatterScalar(content, "abstract", oneLine, {
    insertAt: "end",
  });
}

/**
 * Encode a flow-style list value `tags: [a, b, "c d"]`. Trims each
 * entry, drops empties, dedupes (first wins), quotes entries that
 * contain commas / brackets / leading whitespace / quotes — anything
 * that would re-parse incorrectly otherwise.
 */
function encodeFlowList(values: ReadonlyArray<string>): string {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const raw of values) {
    const trimmed = raw.trim();
    if (trimmed.length === 0) continue;
    if (seen.has(trimmed)) continue;
    seen.add(trimmed);
    out.push(encodeListItem(trimmed));
  }
  return `[${out.join(", ")}]`;
}

function encodeListItem(value: string): string {
  // Items inside a flow-list are split on `,`. Anything that would
  // confuse the parser (commas, square brackets, leading dash that
  // could be read as a YAML scalar marker, surrounding quotes) gets
  // a double-quoted form.
  const needsQuoting =
    /[,[\]"]/.test(value) ||
    /^[-?!&*]/.test(value) ||
    /^\s|\s$/.test(value) ||
    /^["']/.test(value);
  if (!needsQuoting) return value;
  return `"${value.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
}

/**
 * Replace, insert, or remove the `tags:` field. Empty list:
 *   - When the field exists, the line is removed (the user has
 *     cleared all tags via the chip × button).
 *   - When the field is absent, the call is a no-op.
 *
 * Non-empty list: write a flow-style line (`tags: [a, b]`). Insertion
 * happens at the end of `fmBody` so it sits below title + abstract.
 */
export function updateFrontmatterTags(
  content: string,
  tags: ReadonlyArray<string>,
): string {
  const split = splitDoc(content);
  const isEmpty = tags.every((t) => t.trim().length === 0);

  if (!split) {
    if (isEmpty) return content;
    return `---\ntags: ${encodeFlowList(tags)}\n---\n\n${content}`;
  }

  const range = findFieldLineRange(split.fmBody, "tags");

  if (isEmpty) {
    // Removing — drop the existing line (and the newline after it,
    // if any) so we don't leave a blank line behind.
    if (!range) return content;
    const lineEnd = range.to + 1; // include the trailing \n
    const trimmedTo = Math.min(lineEnd, split.fmBody.length);
    const next =
      split.fmBody.slice(0, range.from) + split.fmBody.slice(trimmedTo);
    return split.before + next + split.after;
  }

  const newLine = `tags: ${encodeFlowList(tags)}`;

  if (range) {
    const next =
      split.fmBody.slice(0, range.from) +
      newLine +
      split.fmBody.slice(range.to);
    return split.before + next + split.after;
  }

  const insertion =
    split.fmBody.length === 0 ? `${newLine}\n` : `${split.fmBody}${newLine}\n`;
  return split.before + insertion + split.after;
}

/**
 * Replace, insert, or remove the `tasks:` field. Uses the same
 * empty-list semantics as `updateFrontmatterTags`: empty input drops
 * the line when present, no-ops when absent. Items are serialised via
 * `stringifyTaskItem` (`[ ] foo` / `[x] bar`) and wrapped in a
 * flow-list — staying within the slice-1 schema. The legacy
 * `preflight:` flow-list (M6) was renamed to `tasks:`
 * 9 so the V6 typed pre-flight checks (block-list of kinds) own the
 * `preflight:` key without colliding.
 */
export function updateFrontmatterTasks(
  content: string,
  items: ReadonlyArray<TaskItem>,
): string {
  const split = splitDoc(content);
  const isEmpty = items.length === 0;

  if (!split) {
    if (isEmpty) return content;
    const list = encodeFlowList(items.map(stringifyTaskItem));
    return `---\ntasks: ${list}\n---\n\n${content}`;
  }

  const range = findFieldLineRange(split.fmBody, "tasks");

  if (isEmpty) {
    if (!range) return content;
    const lineEnd = range.to + 1;
    const trimmedTo = Math.min(lineEnd, split.fmBody.length);
    const next =
      split.fmBody.slice(0, range.from) + split.fmBody.slice(trimmedTo);
    return split.before + next + split.after;
  }

  const list = encodeFlowList(items.map(stringifyTaskItem));
  const newLine = `tasks: ${list}`;

  if (range) {
    const next =
      split.fmBody.slice(0, range.from) +
      newLine +
      split.fmBody.slice(range.to);
    return split.before + next + split.after;
  }

  const insertion =
    split.fmBody.length === 0 ? `${newLine}\n` : `${split.fmBody}${newLine}\n`;
  return split.before + insertion + split.after;
}
