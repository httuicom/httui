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
}

/** Parse the frontmatter region into the DocHeader shape (title +
 *  abstract + tags + preflight). Returns an object with `tags: []` /
 *  `preflight: []` and missing optionals when the document has no
 *  frontmatter / no closing fence / unknown keys. Each typed key
 *  follows the Rust slice-1 rule (flow-style only; block scalar
 *  values fall through as `undefined`). */
export function extractFrontmatter(content: string): FrontmatterShape {
  const yaml = splitFrontmatterYaml(content);
  if (yaml === null) return { tags: [], preflight: [] };

  let title: string | undefined;
  let abstractText: string | undefined;
  let tags: string[] = [];
  let preflight: PreflightItem[] = [];
  // Track which top-level keys we've already accepted so duplicate
  // lines (malformed input) take the first occurrence — matches the
  // first-wins shape of the Rust `parse_typed` loop.
  const seen = new Set<string>();

  const lines = yaml.split("\n");
  for (const line of lines) {
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
    } else if (key === "preflight") {
      seen.add(key);
      preflight = parseFlowList(valuePart).map(parsePreflightItem);
    }
  }

  const out: FrontmatterShape = { tags, preflight };
  if (title !== undefined) out.title = title;
  if (abstractText !== undefined) out.abstract = abstractText;
  return out;
}

/** Extract the flow-list `tags:` value from a runbook's YAML
 *  frontmatter. Convenience wrapper around `extractFrontmatter` —
 *  preserved for the per-save tag-index hook which only needs tags. */
export function extractFrontmatterTags(content: string): string[] {
  return extractFrontmatter(content).tags;
}

/** Split the YAML region (between the `---` fences) out of the
 *  document. Returns the raw YAML body without the fences, or
 *  `null` when the document doesn't start with `---\n` / `---\r\n`
 *  or has no closing fence. UTF-8 BOM tolerated. */
function splitFrontmatterYaml(content: string): string | null {
  const stripped = content.startsWith("﻿") ? content.slice(1) : content;
  let rest: string;
  if (stripped.startsWith("---\n")) {
    rest = stripped.slice(4);
  } else if (stripped.startsWith("---\r\n")) {
    rest = stripped.slice(5);
  } else {
    return null;
  }

  const buf: string[] = [];
  while (rest.length > 0) {
    const lineEndAbs = rest.indexOf("\n");
    const lineEnd = lineEndAbs < 0 ? rest.length : lineEndAbs + 1;
    const line = rest.slice(0, lineEnd);
    const lineTrimmed = line.replace(/[\r\n]+$/, "");
    if (lineTrimmed === "---") {
      return buf.join("");
    }
    buf.push(line);
    rest = rest.slice(lineEnd);
  }
  // Hit EOF without seeing the closing fence.
  return null;
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
