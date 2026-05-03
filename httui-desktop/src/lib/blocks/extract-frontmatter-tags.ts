// Epic 50 + Epic 52 — TS frontmatter parser.
//
// Used by:
// - Per-save hook driving `useTagIndexStore.setTagsForFile`
//   (`extractFrontmatterTags`).
// - DocHeaderedEditor mount feeding `<DocHeaderShell frontmatter={…}>`
//   so the H1 falls back to the typed title before reaching the
//   first-heading / filename branches in `pickH1Title`
//   (`extractFrontmatter`).
//
// Synchronous so the consumer can fire it inside the editor save
// callback / render path without an extra IPC round-trip — the Rust
// `httui_core::frontmatter::parse_frontmatter` (252ba21) is the
// authoritative parse on the vault-walker path
// (`scan_vault_tags_cmd`); this is the lightweight per-edit
// counterpart.
//
// Drift contract: this parser must match the Rust slice-1 schema —
// flow-style `tags: [a, b]` only (no block-list); single-line scalar
// `title:` / `abstract:` (block scalar `abstract: |` returns undefined
// here, mirroring `Frontmatter::extra` capture on the Rust side).
// When the Rust parser learns block-list / block-scalar, this helper
// must too. Cross-checked by `parse_flow_list_returns_empty
// _for_block_list_or_other_shapes` in the Rust tests + the matching
// "returns [] on block-list shape" test below.

import {
  parsePreflightItem,
  type PreflightItem,
} from "./preflight-item";

export interface FrontmatterShape {
  title?: string;
  abstract?: string;
  tags: string[];
  /** V2 / cenário 4.5 / M6 — pre-flight checklist items. Stored as a
   *  flow-list of `"[ ] text"` / `"[x] text"` strings to stay within
   *  the slice-1 schema (no block-style nested mappings). */
  preflight: PreflightItem[];
  /** V6 / cenário 6 — user-visible parse error. Set when the
   *  frontmatter region is unterminated (no closing `---`) or when a
   *  typed list key (`tags:` / `preflight:`) carries a non-flow value
   *  the slice-1 schema can't read (block-list shape, bare scalar).
   *  When set, the DocHeader surfaces a "frontmatter invalid" badge so
   *  the user has a visible signal that their YAML didn't apply. */
  error?: string;
}

type SplitResult =
  | { kind: "none" }
  | { kind: "ok"; yaml: string }
  | { kind: "unterminated" };

const ERR_UNTERMINATED =
  "frontmatter inválido: bloco não fechado (faltando `---`)";
const ERR_LIST_NOT_FLOW =
  "frontmatter inválido: `tags` / `preflight` precisam usar flow-style `[a, b]`";

/** Parse the frontmatter region into the DocHeader shape (title +
 *  abstract + tags + preflight). Returns an object with `tags: []` /
 *  `preflight: []` and missing optionals when the document has no
 *  frontmatter / no closing fence / unknown keys. Each typed key
 *  follows the Rust slice-1 rule (flow-style only; block scalar
 *  values fall through as `undefined`). When the YAML is recognizable
 *  but malformed (unterminated / non-flow list), the returned shape
 *  carries `error` so the consumer can surface a badge. */
export function extractFrontmatter(content: string): FrontmatterShape {
  const split = splitFrontmatterYaml(content);
  if (split.kind === "none") return { tags: [], preflight: [] };
  if (split.kind === "unterminated") {
    return { tags: [], preflight: [], error: ERR_UNTERMINATED };
  }

  let title: string | undefined;
  let abstractText: string | undefined;
  let tags: string[] = [];
  let preflight: PreflightItem[] = [];
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
    } else if (key === "preflight") {
      seen.add(key);
      preflight = parseFlowList(valuePart).map(parsePreflightItem);
      if (
        preflight.length === 0 &&
        isMalformedListValue(lines, i, valuePart)
      ) {
        listError = ERR_LIST_NOT_FLOW;
      }
    }
  }

  const out: FrontmatterShape = { tags, preflight };
  if (title !== undefined) out.title = title;
  if (abstractText !== undefined) out.abstract = abstractText;
  if (listError !== null) out.error = listError;
  return out;
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
