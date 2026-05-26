import {
  autocompletion,
  acceptCompletion,
  startCompletion,
  type CompletionContext,
  type CompletionResult,
  type Completion,
} from "@codemirror/autocomplete";
import { keymap, EditorView, tooltips } from "@codemirror/view";
import type { BlockContext } from "./references";

const autocompleteTheme = EditorView.baseTheme({
  ".cm-tooltip": {
    zIndex: "9999 !important",
  },
  ".cm-tooltip-autocomplete li[aria-selected]": {
    background: "var(--chakra-colors-bg-subtle) !important",
    color: "inherit !important",
  },
  ".cm-completionLabel": {
    fontWeight: "500",
    color: "var(--chakra-colors-fg)",
  },
  ".cm-completionDetail": {
    opacity: "0.5",
    marginLeft: "12px",
    fontStyle: "normal !important",
    fontSize: "11px",
  },
  ".cm-completionMatchedText": {
    textDecoration: "none !important",
    fontWeight: "700",
    color: "var(--chakra-colors-fg)",
  },
});

/**
 * Find the start of the current `{{...` expression before the cursor.
 * Returns null if the cursor is not inside a `{{` expression.
 */
function findRefStart(
  text: string,
  pos: number,
): { from: number; inner: string } | null {
  // Look backwards for `{{`
  const before = text.slice(0, pos);
  const openIdx = before.lastIndexOf("{{");
  if (openIdx === -1) return null;

  // Check there's no `}}` between the `{{` and cursor
  const between = before.slice(openIdx);
  if (between.includes("}}")) return null;

  const inner = before.slice(openIdx + 2);
  return { from: openIdx + 2, inner };
}

/** T28: Max depth for autocomplete traversal to prevent deep structure exposure. */
const MAX_AUTOCOMPLETE_DEPTH = 5;

/** T32: Dangerous property names blocked from autocomplete. */
const DANGEROUS_KEYS = new Set(["__proto__", "constructor", "prototype"]);

/**
 * Navigate into a JSON value to get keys at a given path.
 */
function getKeysAtPath(data: unknown, pathParts: string[]): Completion[] {
  // T28: Limit autocomplete depth
  if (pathParts.length > MAX_AUTOCOMPLETE_DEPTH) return [];

  let current = data;

  for (const part of pathParts) {
    if (current == null) return [];

    if (Array.isArray(current)) {
      const idx = parseInt(part, 10);
      if (isNaN(idx) || idx < 0 || idx >= current.length) return [];
      current = current[idx];
    } else if (typeof current === "object") {
      if (DANGEROUS_KEYS.has(part)) return [];
      const obj = current as Record<string, unknown>;
      if (!(part in obj)) return [];
      current = obj[part];
    } else {
      return [];
    }
  }

  if (current == null) return [];

  if (Array.isArray(current)) {
    return current.map((_, i) => ({
      label: String(i),
      type: "property",
      detail: summarizeValue(current[i]),
    }));
  }

  if (typeof current === "object") {
    return Object.entries(current as Record<string, unknown>)
      .filter(([key]) => !DANGEROUS_KEYS.has(key))
      .map(([key, val]) => ({
        label: key,
        type: "property",
        detail: summarizeValue(val),
      }));
  }

  return [];
}

function summarizeValue(val: unknown): string {
  if (val == null) return "null";
  if (typeof val === "string")
    return val.length > 30 ? `"${val.slice(0, 30)}..."` : `"${val}"`;
  if (typeof val === "number" || typeof val === "boolean") return String(val);
  if (Array.isArray(val)) return `Array(${val.length})`;
  if (typeof val === "object") return `{${Object.keys(val).length} keys}`;
  return typeof val;
}

/**
 * Creates a CodeMirror autocompletion extension for `{{...}}` block references.
 * Pass a getter that returns the current block contexts (aliases + cached results).
 */
/**
 * Creates just the completion source function for `{{...}}` references.
 * Use this when you need to combine with other completion sources (e.g., SQL).
 */
export interface EnvKeyInfo {
  key: string;
  isSecret: boolean;
}

export function createReferenceCompletionSource(
  getBlocks: () => BlockContext[],
  getEnvKeys?: () => (string | EnvKeyInfo)[],
): (ctx: CompletionContext) => CompletionResult | null {
  return (ctx: CompletionContext) => {
    const line = ctx.state.doc.lineAt(ctx.pos);
    const textBefore = line.text.slice(0, ctx.pos - line.from);
    const ref = findRefStart(textBefore, textBefore.length);

    if (!ref) return null;

    const from = line.from + ref.from;
    const inner = ref.inner;
    const blocks = getBlocks();

    if (!inner.includes(".")) {
      const options: Completion[] = blocks
        .filter((b) => b.alias)
        .map((b) => ({
          label: b.alias,
          type: "variable",
          detail: b.cachedResult ? "cached" : "no result",
        }));

      // T29: Filter secret env keys from autocomplete suggestions
      if (getEnvKeys) {
        for (const entry of getEnvKeys()) {
          const key = typeof entry === "string" ? entry : entry.key;
          const isSecret = typeof entry === "string" ? false : entry.isSecret;
          if (!isSecret) {
            options.push({
              label: key,
              type: "variable",
              detail: "env",
            });
          }
        }
      }

      return { from, to: ctx.pos, options, filter: true };
    }

    const parts = inner.split(".");
    const alias = parts[0];
    const block = blocks.find((b) => b.alias === alias);
    if (!block?.cachedResult) return null;

    let responseData: unknown;
    try {
      responseData = JSON.parse(block.cachedResult.response);
    } catch {
      return null;
    }

    const context: Record<string, unknown> = {
      response: responseData,
      status: block.cachedResult.status,
    };

    const completedPath = parts.slice(1, -1);
    const options = getKeysAtPath(context, completedPath);
    if (options.length === 0) return null;

    const lastDotIdx = inner.lastIndexOf(".");
    const completionFrom = line.from + ref.from + lastDotIdx + 1;

    return { from: completionFrom, to: ctx.pos, options, filter: true };
  };
}

export function createReferenceAutocomplete(
  getBlocks: () => BlockContext[],
  getEnvKeys?: () => (string | EnvKeyInfo)[],
) {
  function completionSource(ctx: CompletionContext): CompletionResult | null {
    const line = ctx.state.doc.lineAt(ctx.pos);
    const textBefore = line.text.slice(0, ctx.pos - line.from);
    const ref = findRefStart(textBefore, textBefore.length);

    if (!ref) return null;

    const from = line.from + ref.from;
    const inner = ref.inner; // e.g. "" or "login" or "login.response" or "login.response.body."

    const blocks = getBlocks();

    // No dot yet — complete alias names + env variable keys
    if (!inner.includes(".")) {
      const options: Completion[] = blocks
        .filter((b) => b.alias)
        .map((b) => ({
          label: b.alias,
          type: "variable",
          detail: b.cachedResult ? "cached" : "no result",
        }));

      // T29: Filter secret env keys from autocomplete suggestions
      if (getEnvKeys) {
        for (const entry of getEnvKeys()) {
          const key = typeof entry === "string" ? entry : entry.key;
          const isSecret = typeof entry === "string" ? false : entry.isSecret;
          if (!isSecret) {
            options.push({
              label: key,
              type: "variable",
              detail: "env",
            });
          }
        }
      }

      return {
        from,
        to: ctx.pos,
        options,
        filter: true,
      };
    }

    // Has dots — complete JSON path
    const parts = inner.split(".");
    const alias = parts[0];
    const block = blocks.find((b) => b.alias === alias);
    if (!block?.cachedResult) return null;

    let responseData: unknown;
    try {
      responseData = JSON.parse(block.cachedResult.response);
    } catch {
      return null;
    }

    // Build context object matching what resolveReference uses
    const context: Record<string, unknown> = {
      response: responseData,
      status: block.cachedResult.status,
    };

    // Path parts after alias, excluding the last (being typed)
    const completedPath = parts.slice(1, -1);
    const options = getKeysAtPath(context, completedPath);

    if (options.length === 0) return null;

    // `from` should be after the last dot
    const lastDotIdx = inner.lastIndexOf(".");
    const completionFrom = line.from + ref.from + lastDotIdx + 1;

    return {
      from: completionFrom,
      to: ctx.pos,
      options,
      filter: true,
    };
  }

  return [
    autocompletion({
      override: [completionSource],
      activateOnTyping: true,
    }),
    tooltips({ parent: document.body }),
    keymap.of([
      { key: "Ctrl-Space", run: startCompletion },
      { key: "Tab", run: acceptCompletion },
    ]),
    autocompleteTheme,
  ];
}
