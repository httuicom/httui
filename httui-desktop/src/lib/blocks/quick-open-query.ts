// Quick-open query parser (Epic 52 Story 05).
//
// `Cmd+P` lets the user type either:
//   - a fuzzy file-name query: `apt purr` → fuzzy match against the
//     vault's `.md` set (existing `search_files` Tauri cmd path)
//   - a tag query: `#payments` → exact tag match against
//     `useTagIndexStore.getFilesByTag(tag)` (Epic 52 Story 04 store)
//   - boolean tag combo (stretch): `#payments OR #debug` →
//     union of both sets; `#payments AND #debug` → intersection
//
// Pure parsing only — no I/O, no filesystem. The consumer hook
// (`useFileSearch`) routes to the right backend based on `kind`.

export type QuickOpenQuery =
  | { kind: "empty" }
  | { kind: "fuzzy"; value: string }
  | { kind: "tag"; tag: string }
  | { kind: "tag-bool"; op: "or" | "and"; tags: string[] };

const TAG_PATTERN = /^#([A-Za-z0-9_][A-Za-z0-9_-]*)$/;

/** Parse the user's quick-open input. Empty / whitespace returns
 *  `empty`; the consumer can then surface "recent files" or similar
 *  default. Otherwise routes to fuzzy or tag-mode based on the
 *  `#`-prefix. Boolean tag combos require the literal keywords
 *  `OR` / `AND` (uppercase or lowercase) between two tag tokens. */
export function parseQuickOpenQuery(raw: string): QuickOpenQuery {
  const trimmed = raw.trim();
  if (!trimmed) return { kind: "empty" };

  // Boolean combos take priority — only pattern with multiple tag
  // tokens. Tokens are split on whitespace; we accept any number
  // of `#tag` tokens joined by a single shared OR/AND keyword.
  const tokens = trimmed.split(/\s+/);
  if (tokens.length >= 3 && tokens.length % 2 === 1) {
    const operators = tokens.filter((_, i) => i % 2 === 1);
    const operands = tokens.filter((_, i) => i % 2 === 0);
    const ops = new Set(operators.map((o) => o.toLowerCase()));
    const allTagTokens = operands.every((o) => TAG_PATTERN.test(o));
    if (allTagTokens && ops.size === 1 && (ops.has("or") || ops.has("and"))) {
      const op = ops.has("or") ? "or" : "and";
      const tags = operands.map((o) => o.replace(/^#/, ""));
      return { kind: "tag-bool", op, tags };
    }
  }

  // Single tag token.
  if (tokens.length === 1) {
    const m = TAG_PATTERN.exec(tokens[0]);
    if (m) {
      return { kind: "tag", tag: m[1] };
    }
  }

  return { kind: "fuzzy", value: trimmed };
}

/** Apply a parsed tag query against the index. `byTag(tag)` is the
 *  store accessor (`useTagIndexStore.getFilesByTag`); we keep this
 *  pure by injecting it. Returns files in the order they appear
 *  in the first matching tag's set, then de-duplicated for
 *  subsequent tags (OR mode). For `and` mode, returns the
 *  intersection in first-tag order. */
export function applyTagQuery(
  query: QuickOpenQuery,
  byTag: (tag: string) => string[],
): string[] {
  if (query.kind === "tag") {
    return [...byTag(query.tag)];
  }
  if (query.kind === "tag-bool") {
    if (query.tags.length === 0) return [];
    const sets = query.tags.map((t) => byTag(t));
    if (query.op === "or") {
      const seen = new Set<string>();
      const out: string[] = [];
      for (const set of sets) {
        for (const path of set) {
          if (!seen.has(path)) {
            seen.add(path);
            out.push(path);
          }
        }
      }
      return out;
    }
    // and-mode: keep paths present in every set, in first-set order.
    const [first, ...rest] = sets;
    const restSets = rest.map((s) => new Set(s));
    return first.filter((p) => restSets.every((s) => s.has(p)));
  }
  return [];
}
