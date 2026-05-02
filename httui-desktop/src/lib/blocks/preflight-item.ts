// V2 / cenário 4.5 / M6 — pre-flight checklist item shape and the
// flow-style YAML round-trip helpers used by the editable checklist
// in the DocHeader.
//
// The slice-1 frontmatter parser only supports single-line scalars and
// flow-style lists, so the checklist is stored as a flow-list of
// strings prefixed with `[ ]` / `[x]` markers (the same convention
// markdown task lists use). Example:
//
//   preflight: ["[ ] Verify connection", "[x] Set keychain"]
//
// Pure functions: text in / text out. The DocHeader render +
// updateFrontmatterPreflight writer compose them.

export interface PreflightItem {
  text: string;
  done: boolean;
}

const ITEM_PREFIX_RE = /^\[\s*(x|X|\s?)\s*\]\s*(.*)$/;

/**
 * Parse one flow-list entry into a `PreflightItem`. Entries without a
 * `[x]` / `[ ]` prefix default to `done: false` so the helper is
 * tolerant of hand-edited YAML where the user just wrote a label.
 */
export function parsePreflightItem(raw: string): PreflightItem {
  const trimmed = raw.trim();
  const match = trimmed.match(ITEM_PREFIX_RE);
  if (match) {
    const flag = match[1];
    return { text: match[2].trim(), done: flag === "x" || flag === "X" };
  }
  return { text: trimmed, done: false };
}

/**
 * Format a `PreflightItem` back into its flow-list string form. Always
 * emits the `[x]` / `[ ]` prefix so subsequent parses round-trip
 * cleanly.
 */
export function stringifyPreflightItem(item: PreflightItem): string {
  const flag = item.done ? "x" : " ";
  return `[${flag}] ${item.text}`;
}
