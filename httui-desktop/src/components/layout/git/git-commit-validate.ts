export interface CommitValidation {
  valid: boolean;
  errors: ReadonlyArray<string>;
  /** Subject (first line) extracted from the message. Empty when blank. */
  subject: string;
  /** Body (everything after the first blank line). */
  body: string;
}

/** Conservative subject-line length cap. The git convention is 50;
 *  we accept up to 72 to match the .gitmessage default + GitHub
 *  truncation. Overshoots warn the user but don't block validation. */
export const SUBJECT_MAX_LENGTH = 72;

export function validateCommitMessage(message: string): CommitValidation {
  const errors: string[] = [];
  const trimmed = message.replace(/\s+$/u, "");

  if (trimmed.trim().length === 0) {
    errors.push("Commit message cannot be empty.");
    return { valid: false, errors, subject: "", body: "" };
  }

  // Body starts after the first blank line (git convention); when absent, body is empty.
  const lines = trimmed.split("\n");
  const subject = lines[0]!.replace(/^\s+/u, "");
  const blankIdx = lines.findIndex((l, i) => i > 0 && l.trim().length === 0);
  const body =
    blankIdx === -1
      ? ""
      : lines
          .slice(blankIdx + 1)
          .join("\n")
          .trim();

  if (subject.length === 0) {
    errors.push("Commit subject (first line) cannot be empty.");
  }
  if (subject.length > SUBJECT_MAX_LENGTH) {
    errors.push(
      `Commit subject is ${subject.length} chars; keep it under ${SUBJECT_MAX_LENGTH}.`,
    );
  }
  if (lines[0] !== subject) {
    errors.push("Commit subject should not have leading whitespace.");
  }

  return {
    valid: errors.length === 0,
    errors,
    subject,
    body,
  };
}

/** "N file" / "N files" — used by the commit form footer. */
export function pluralizeFiles(n: number): string {
  return n === 1 ? `${n} file` : `${n} files`;
}
