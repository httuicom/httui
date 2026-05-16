// V10.1 cenário 2 + 8 — derive the pre-filled commit message from
// the changed-files list.
//
// Default (no template configured): a sensible conditional —
//   0 changes → ""               (nothing to commit)
//   1 change  → "Update <stem>"
//   N changes → "Update N notes"
//
// Configurable (user.toml `[ui].git_commit_template`): the string
// is rendered with placeholders so a user/team can pin their own
// convention:
//   {{notes}} → changed note stems, comma-joined
//   {{count}} → number of changed files
//   {{date}}  → YYYY-MM-DD (local)
//
// Pure + framework-free so it unit-tests without a DOM and feeds
// both the side panel prefill effect and its tests.

/** Basename without a trailing `.md` (notes are markdown). Other
 *  extensions keep their full filename so config changes still read
 *  sensibly. */
export function noteStem(path: string): string {
  const base = path.split("/").pop() ?? path;
  return base.endsWith(".md") ? base.slice(0, -3) : base;
}

function pad2(n: number): string {
  return n < 10 ? `0${n}` : String(n);
}

function isoDate(d: Date): string {
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
}

export function deriveCommitMessage(
  changedPaths: ReadonlyArray<string>,
  template: string | null | undefined,
  now: Date = new Date(),
): string {
  const stems = changedPaths.map(noteStem);
  const count = stems.length;

  const tpl = template?.trim();
  if (tpl) {
    return tpl
      .replace(/\{\{\s*notes\s*\}\}/g, stems.join(", "))
      .replace(/\{\{\s*count\s*\}\}/g, String(count))
      .replace(/\{\{\s*date\s*\}\}/g, isoDate(now));
  }

  if (count === 0) return "";
  if (count === 1) return `Update ${stems[0]}`;
  return `Update ${count} notes`;
}
