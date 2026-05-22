// Checklist task shape and YAML round-trip helpers for the DocHeader.
//
// Stored under `tasks:` as a flow-list of `"[ ] text"` / `"[x] text"` strings.
// Slice-1 only supports flow-style lists:
//
//   tasks: ["[ ] Verify connection", "[x] Set keychain"]
//
// Pure functions: text in / text out.

export interface TaskItem {
  text: string;
  done: boolean;
}

const ITEM_PREFIX_RE = /^\[\s*(x|X|\s?)\s*\]\s*(.*)$/;

/**
 * Parse one flow-list entry into a `TaskItem`. Entries without a
 * `[x]` / `[ ]` prefix default to `done: false` so the helper is
 * tolerant of hand-edited YAML where the user just wrote a label.
 */
export function parseTaskItem(raw: string): TaskItem {
  const trimmed = raw.trim();
  const match = trimmed.match(ITEM_PREFIX_RE);
  if (match) {
    const flag = match[1];
    return { text: match[2].trim(), done: flag === "x" || flag === "X" };
  }
  return { text: trimmed, done: false };
}

/**
 * Format a `TaskItem` back into its flow-list string form. Always
 * emits the `[x]` / `[ ]` prefix so subsequent parses round-trip
 * cleanly.
 */
export function stringifyTaskItem(item: TaskItem): string {
  const flag = item.done ? "x" : " ";
  return `[${flag}] ${item.text}`;
}
