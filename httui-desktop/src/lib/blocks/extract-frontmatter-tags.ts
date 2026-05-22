// Lightweight TS frontmatter parser — synchronous counterpart to
// `httui_core::frontmatter::parse_frontmatter`.
//
// Used by the per-save tag-index hook and DocHeader title/abstract display.
//
// Drift contract: must match the Rust slice-1 schema — flow-style
// `tags: [a, b]` only, single-line scalars. When the Rust parser gains
// block-list or block-scalar support, this helper must follow.

import { parseTaskItem, type TaskItem } from "./task-item";

export interface FrontmatterShape {
  title?: string;
  abstract?: string;
  tags: string[];
  /** Free-form checklist items stored under `tasks:` as a flow-list. */
  tasks: TaskItem[];
  /** `status:` value (`draft` | `active` | `archived`, forward-compat).
   *  `archived` hides the note from the default file tree view. */
  status?: string;
  /** User-visible parse error: unterminated frontmatter or non-flow list value. */
  error?: string;
}

type SplitResult =
  | { kind: "none" }
  | { kind: "ok"; yaml: string }
  | { kind: "unterminated" };

const ERR_UNTERMINATED =
  "frontmatter inválido: bloco não fechado (faltando `---`)";
const ERR_LIST_NOT_FLOW =
  "frontmatter inválido: `tags` / `tasks` precisam usar flow-style `[a, b]`";

/** Parse the frontmatter region into the DocHeader shape (title +
 *  abstract + tags + tasks). Returns an object with `tags: []` /
 *  `tasks: []` and missing optionals when the document has no
 *  frontmatter / no closing fence / unknown keys. Each typed key
 *  follows the Rust slice-1 rule (flow-style only; block scalar
 *  values fall through as `undefined`). When the YAML is recognizable
 *  but malformed (unterminated / non-flow list), the returned shape
 *  carries `error` so the consumer can surface a badge. */
export function extractFrontmatter(content: string): FrontmatterShape {
  const split = splitFrontmatterYaml(content);
  if (split.kind === "none") return { tags: [], tasks: [] };
  if (split.kind === "unterminated") {
    return { tags: [], tasks: [], error: ERR_UNTERMINATED };
  }

  let title: string | undefined;
  let abstractText: string | undefined;
  let tags: string[] = [];
  let tasks: TaskItem[] = [];
  let status: string | undefined;
  let listError: string | null = null;
  // Track which top-level keys we've already accepted so duplicate
  // lines (malformed input) take the first occurrence — matches the
  // first-wins shape of the Rust `parse_typed` loop.
  const seen = new Set<string>();

  const lines = split.yaml.split("\n");
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]!;
    if (line.startsWith(" ") || line.startsWith("\t")) continue;
    const trimmed = line.replace(/[\r\n]+$/, "");
    if (trimmed === "" || trimmed.startsWith("#")) continue;
    const colonIdx = trimmed.indexOf(":");
    if (colonIdx < 0) continue;
    const key = trimmed.slice(0, colonIdx).trim();
    if (seen.has(key)) continue;
    const valuePart = trimmed.slice(colonIdx + 1).trim();

    if (key === "title") {
      seen.add(key);
      const u = unquote(valuePart);
      if (u !== "") title = u;
    } else if (key === "status") {
      seen.add(key);
      const u = unquote(valuePart).trim().toLowerCase();
      if (u !== "") status = u;
    } else if (key === "abstract") {
      seen.add(key);
      // Block-scalar marker (`abstract: |` / `abstract: >`) — leave
      // undefined (Rust slice-1 also defers).
      if (valuePart === "|" || valuePart === ">") continue;
      const u = unquote(valuePart);
      if (u !== "") abstractText = u;
    } else if (key === "tags") {
      seen.add(key);
      tags = parseFlowList(valuePart);
      if (tags.length === 0 && isMalformedListValue(lines, i, valuePart)) {
        listError = ERR_LIST_NOT_FLOW;
      }
    } else if (key === "tasks") {
      seen.add(key);
      tasks = parseFlowList(valuePart).map(parseTaskItem);
      if (tasks.length === 0 && isMalformedListValue(lines, i, valuePart)) {
        listError = ERR_LIST_NOT_FLOW;
      }
    }
  }

  const out: FrontmatterShape = { tags, tasks };
  if (title !== undefined) out.title = title;
  if (abstractText !== undefined) out.abstract = abstractText;
  if (status !== undefined) out.status = status;
  if (listError !== null) out.error = listError;
  return out;
}

/** convenience wrapper used by `useTagIndexStore` to
 *  drive the file-tree archived filter on save. Equivalent to
 *  `extractFrontmatter(content).status === "archived"`. */
export function extractFrontmatterArchived(content: string): boolean {
  return extractFrontmatter(content).status === "archived";
}

/** A list-shaped key (`tags:` / `preflight:`) is malformed when:
 *  - its inline value is non-empty AND doesn't match the flow-list
 *    grammar (`[a, b]`) — e.g. `tags: foo` or `tags: "x"` (the slice-1
 *    schema rejects bare scalars / quoted scalars in list slots), OR
 *  - its inline value is empty AND the next non-blank/non-comment line
 *    is indented + starts with `-` (block-list shape — also outside
 *    slice-1).
 *  Empty value followed by another top-level key / EOF is benign
 *  (the user typed `tags:` and is about to add a value). */
function isMalformedListValue(
  lines: string[],
  keyLineIndex: number,
  valuePart: string,
): boolean {
  const v = valuePart.trim();
  if (v.length > 0) {
    if (v.startsWith("[") && v.endsWith("]")) return false;
    return true;
  }
  for (let j = keyLineIndex + 1; j < lines.length; j++) {
    const next = lines[j]!.replace(/[\r\n]+$/, "");
    if (next === "") continue;
    if (next.startsWith("#")) continue;
    const indented = next.startsWith(" ") || next.startsWith("\t");
    if (!indented) return false;
    return /^[ \t]+-(\s|$)/.test(next);
  }
  return false;
}

/** Extract the flow-list `tags:` value from a runbook's YAML
 *  frontmatter. Convenience wrapper around `extractFrontmatter` —
 *  preserved for the per-save tag-index hook which only needs tags. */
export function extractFrontmatterTags(content: string): string[] {
  return extractFrontmatter(content).tags;
}

/** Split the YAML region (between the `---` fences) out of the
 *  document. Returns `kind: "none"` when the doc doesn't start with a
 *  fence, `kind: "ok"` with the raw YAML body, or `kind: "unterminated"`
 *  when the open fence isn't matched. UTF-8 BOM tolerated. The
 *  unterminated case is what the consumer surfaces as
 *  "frontmatter inválido". */
function splitFrontmatterYaml(content: string): SplitResult {
  const stripped = content.startsWith("\u{feff}") ? content.slice(1) : content;
  let rest: string;
  if (stripped.startsWith("---\n")) {
    rest = stripped.slice(4);
  } else if (stripped.startsWith("---\r\n")) {
    rest = stripped.slice(5);
  } else {
    return { kind: "none" };
  }

  const buf: string[] = [];
  while (rest.length > 0) {
    const lineEndAbs = rest.indexOf("\n");
    const lineEnd = lineEndAbs < 0 ? rest.length : lineEndAbs + 1;
    const line = rest.slice(0, lineEnd);
    const lineTrimmed = line.replace(/[\r\n]+$/, "");
    if (lineTrimmed === "---") {
      return { kind: "ok", yaml: buf.join("") };
    }
    buf.push(line);
    rest = rest.slice(lineEnd);
  }
  // Hit EOF without seeing the closing fence.
  return { kind: "unterminated" };
}

/** Parse a flow-style list `[a, b, "c"]`. Returns `[]` on any other
 *  shape. Quoted strings (single or double) get unquoted; entries
 *  that trim to empty are filtered. Duplicates removed. */
function parseFlowList(value: string): string[] {
  const v = value.trim();
  if (!v.startsWith("[") || !v.endsWith("]")) return [];
  const inner = v.slice(1, -1);
  const out: string[] = [];
  const seen = new Set<string>();
  for (const item of inner.split(",")) {
    const unquoted = unquote(item.trim());
    if (unquoted === "") continue;
    if (seen.has(unquoted)) continue;
    seen.add(unquoted);
    out.push(unquoted);
  }
  return out;
}

function unquote(value: string): string {
  const v = value.trim();
  if (v.length < 2) return v;
  const first = v[0];
  const last = v[v.length - 1];
  if ((first === '"' && last === '"') || (first === "'" && last === "'")) {
    return v.slice(1, -1);
  }
  return v;
}
