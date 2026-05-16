// V10.1 cenário 3 (polish) — make raw git stderr readable.
//
// `git push` failures arrive as a single run-on string full of
// `remote:` prefixes (GitHub branch-protection / rule-violation
// messages are the worst offenders). We keep the verbatim git text
// (truth) but: split it back into lines, drop the `remote:` noise,
// and derive a short human headline for the common rejections.
//
// Pure + framework-free so it unit-tests without a DOM.

export interface FormattedGitError {
  /** One-line, human, actionable headline. */
  summary: string;
  /** The cleaned multi-line git output (verbatim content, just
   *  de-noised and re-broken into lines). */
  detail: string;
}

const SUMMARY_RULES: ReadonlyArray<{ re: RegExp; summary: string }> = [
  {
    re: /protected branch|repository rule violations|must be made through a pull request|push declined|GH013|GH006/i,
    summary: "Push rejected — this branch requires a pull request.",
  },
  {
    re: /non-fast-forward|fetch first|tip of your current branch is behind|\[rejected\][^\n]*\(fetch first\)/i,
    summary: "Push rejected — your branch is behind. Pull first.",
  },
  {
    re: /could not read Username|Authentication failed|Permission denied \(publickey\)|terminal prompts disabled|Invalid username or password/i,
    summary: "Authentication failed — check your git credentials or SSH key.",
  },
  {
    re: /not a git repository/i,
    summary: "Not a git repository.",
  },
  {
    re: /Commit message is empty/i,
    summary: "Commit message is empty.",
  },
  {
    re: /CONFLICT|Automatic merge failed|needs merge|fix conflicts/i,
    summary: "Merge conflict — resolve it in the detailed panel.",
  },
];

/** Break the run-on into readable lines: split on the `remote:`
 *  prefix and on the git boundary tokens git itself would have put
 *  on their own lines, then trim/dedupe. */
function toLines(raw: string): string[] {
  const broken = raw
    .replace(/\r/g, "")
    .replace(/\s*remote:\s*/g, "\n")
    .replace(/\s+(?=! \[)/g, "\n")
    .replace(/\s+(?=To )/g, "\n")
    .replace(/\s+(?=error: )/g, "\n")
    .replace(/\s+(?=hint: )/g, "\n");

  const out: string[] = [];
  for (const piece of broken.split("\n")) {
    const line = piece.replace(/\s{2,}/g, " ").trim();
    if (line && out[out.length - 1] !== line) out.push(line);
  }
  return out;
}

export function formatGitError(raw: string): FormattedGitError {
  const lines = toLines(raw);
  const detail = lines.join("\n");

  const matched = SUMMARY_RULES.find((r) => r.re.test(raw));
  // No known pattern — lead with the first informative line.
  const fallback = lines.find((l) => !/^To\b/.test(l)) ?? "";
  const summary = matched?.summary ?? (fallback || "Git command failed.");

  return { summary, detail };
}
