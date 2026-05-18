// Pure helpers for the empty-state ⌘V paste-URL flow. Two responsibilities:
//
// - `extractUrl(text)`: detect when a pasted clipboard payload should
//   trigger the scaffold-with-block path. Strict so accidental text
//   (containing a URL fragment) doesn't hijack the paste — only when
//   the entire trimmed payload is a valid http(s) URL.
// - `buildRunbookFromUrl(url)`: produce the markdown body for a fresh
//   runbook with a single executable HTTP `GET <url>` block. Re-uses
//   the same fence shape as `BLOCK_TEMPLATES.http` so the runbook is
//   immediately runnable inside the editor.

const URL_RE = /^https?:\/\/\S+$/i;

/**
 * Returns the trimmed URL when `text` is **only** an http(s) URL —
 * `null` otherwise. The trim handles trailing newlines / spaces from
 * normal copy-paste sources (browser address bar, terminals).
 */
export function extractUrl(text: string): string | null {
  if (!text) return null;
  const trimmed = text.trim();
  if (!URL_RE.test(trimmed)) return null;
  // Reject control characters that would break the markdown fence.
  if (/[\r\n]/.test(trimmed)) return null;
  return trimmed;
}

/**
 * Build the markdown body for a brand-new runbook scaffolded from a
 * pasted URL. Keeps the same `\`\`\`http alias=req1\nGET …\n\`\`\``
 * shape as `BLOCK_TEMPLATES.http` so the block is immediately
 * executable. Title is "Untitled runbook" — the user renames in the
 * file tree.
 */
export function buildRunbookFromUrl(url: string): string {
  return `# Untitled runbook\n\n\`\`\`http alias=req1\nGET ${url}\n\`\`\`\n`;
}

/**
 * Default vault-relative path for the runbook the paste flow writes.
 * Lives under `runbooks/` because that's the canonical scaffold
 * location (`scaffold_new_vault` always creates the dir).
 */
export const PASTE_URL_RUNBOOK_PATH = "runbooks/untitled.md";
