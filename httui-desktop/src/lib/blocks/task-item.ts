// / M6 — checklist task shape and the flow-style
// YAML round-trip helpers used by the editable checklist in the
// DocHeader.
//
// split: this used to live under the YAML key `preflight:`
// which now belongs to the V6 pre-flight checks (block-list of typed
// kinds — connection / env_var / branch / keychain / file_exists /
// command). The free-form checklist moved to `tasks:` so the two
// schemas stop colliding and the V6 builder UI can edit the typed
// kinds without touching the user's todo list.
//
// The slice-1 frontmatter parser only supports single-line scalars and
// flow-style lists, so the checklist is stored as a flow-list of
// strings prefixed with `[ ]` / `[x]` markers (the same convention
// markdown task lists use). Example:
//
//   tasks: ["[ ] Verify connection", "[x] Set keychain"]
//
// Pure functions: text in / text out. The DocHeader render +
// `updateFrontmatterTasks` writer compose them.

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
