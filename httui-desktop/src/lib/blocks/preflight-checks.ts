// V6 / cenário 9 — TS reader + writer for the block-list `preflight:`
// section in YAML frontmatter. Mirrors `httui_core::preflight::parser`
// (Rust) so the editor can mutate pre-flight checks visually without
// going through the Tauri evaluator.
//
// Schema:
//
//   preflight:
//     - connection: payments-db
//     - env_var: API_TOKEN
//     - branch: main
//     - keychain: payments-db.password
//     - file_exists: ./creds/api.json
//     - command: psql --version
//
// Pure string-in / string-out. The DocHeader builder UI calls
// `extractPreflightChecks` on render and `updateFrontmatterPreflightChecks`
// on every add / edit / remove.

export type PreflightCheckKind =
  | "connection"
  | "env_var"
  | "branch"
  | "keychain"
  | "file_exists"
  | "command";

export interface PreflightCheck {
  kind: PreflightCheckKind;
  /** Single-string value: connection name / env var name / branch
   *  name / keychain key / file path / command. The YAML maps this
   *  to the per-kind label (`connection: foo` / `file_exists: ./x`). */
  value: string;
}

const KIND_KEYS: ReadonlySet<PreflightCheckKind> = new Set([
  "connection",
  "env_var",
  "branch",
  "keychain",
  "file_exists",
  "command",
]);

interface SplitDoc {
  before: string;
  fmBody: string;
  after: string;
}

function splitDoc(content: string): SplitDoc | null {
  const stripped = content.startsWith("\u{FEFF}") ? content.slice(1) : content;
  let openFenceLen: number;
  if (stripped.startsWith("---\n")) {
    openFenceLen = 4;
  } else if (stripped.startsWith("---\r\n")) {
    openFenceLen = 5;
  } else {
    return null;
  }

  const before = stripped.slice(0, openFenceLen);
  let rest = stripped.slice(openFenceLen);
  const buf: string[] = [];
  while (rest.length > 0) {
    const lineEndAbs = rest.indexOf("\n");
    const lineEnd = lineEndAbs < 0 ? rest.length : lineEndAbs + 1;
    const line = rest.slice(0, lineEnd);
    const trimmedLine = line.replace(/[\r\n]+$/, "");
    if (trimmedLine === "---") {
      const after = line + rest.slice(lineEnd);
      return { before, fmBody: buf.join(""), after };
    }
    buf.push(line);
    rest = rest.slice(lineEnd);
  }
  return null;
}

/** Read the typed `preflight:` items out of a markdown document.
 *  Returns `[]` when no frontmatter / no `preflight:` block / empty
 *  block. Tolerates indentation variations (any whitespace before the
 *  `-` marker) and quoted values. */
export function extractPreflightChecks(content: string): PreflightCheck[] {
  const split = splitDoc(content);
  if (!split) return [];
  return parsePreflightFromYaml(split.fmBody);
}

function parsePreflightFromYaml(rawYaml: string): PreflightCheck[] {
  const lines = rawYaml.split("\n");
  const out: PreflightCheck[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i]!;
    const trimmed = line.replace(/[\r\n]+$/, "");
    if (trimmed === "preflight:") {
      i++;
      while (i < lines.length) {
        const next = lines[i]!.replace(/[\r\n]+$/, "");
        if (next === "") {
          i++;
          continue;
        }
        if (!next.startsWith(" ") && !next.startsWith("\t")) {
          break;
        }
        const item = parseItem(next);
        if (item) out.push(item);
        i++;
      }
      // Only the first `preflight:` header counts — subsequent
      // duplicates are out-of-spec; bail.
      return out;
    }
    i++;
  }
  return out;
}

function parseItem(line: string): PreflightCheck | null {
  const trimmed = line.trim();
  if (!trimmed.startsWith("- ") && trimmed !== "-") return null;
  const body = trimmed === "-" ? "" : trimmed.slice(2).trim();
  if (body.length === 0) return null;
  const colonIdx = body.indexOf(":");
  if (colonIdx < 0) return null;
  const key = body.slice(0, colonIdx).trim();
  const value = unquote(body.slice(colonIdx + 1).trim());
  if (key.length === 0 || value.length === 0) return null;
  if (!KIND_KEYS.has(key as PreflightCheckKind)) return null;
  return { kind: key as PreflightCheckKind, value };
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

/** Replace, insert, or remove the typed block-list `preflight:`
 *  field. Empty input drops the section when present, no-ops when
 *  absent. Items render as `  - {kind}: {value}` two-space indented;
 *  values that contain YAML-special characters are double-quoted. */
export function updateFrontmatterPreflightChecks(
  content: string,
  checks: ReadonlyArray<PreflightCheck>,
): string {
  const split = splitDoc(content);
  const isEmpty = checks.length === 0;

  if (!split) {
    if (isEmpty) return content;
    return `---\n${formatPreflightBlock(checks)}---\n\n${content}`;
  }

  const range = findPreflightBlockRange(split.fmBody);

  if (isEmpty) {
    if (!range) return content;
    const next =
      split.fmBody.slice(0, range.from) + split.fmBody.slice(range.to);
    return split.before + next + split.after;
  }

  const block = formatPreflightBlock(checks);

  if (range) {
    const next =
      split.fmBody.slice(0, range.from) + block + split.fmBody.slice(range.to);
    return split.before + next + split.after;
  }

  // Insert at the end of the frontmatter body.
  const insertion =
    split.fmBody.length === 0 ? block : `${split.fmBody}${block}`;
  return split.before + insertion + split.after;
}

function formatPreflightBlock(checks: ReadonlyArray<PreflightCheck>): string {
  const lines = ["preflight:"];
  for (const check of checks) {
    lines.push(`  - ${check.kind}: ${encodeValue(check.value)}`);
  }
  return lines.join("\n") + "\n";
}

function encodeValue(value: string): string {
  if (value.length === 0) return '""';
  const needsQuoting =
    /[:#"\\\n]/.test(value) ||
    /^[-?!&*]/.test(value) ||
    /^\s|\s$/.test(value) ||
    value === "~" ||
    value === "null" ||
    value === "true" ||
    value === "false" ||
    /^-?\d/.test(value);
  if (!needsQuoting) return value;
  return `"${value.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
}

interface BlockRange {
  from: number;
  to: number;
}

/** Locate the `preflight:` block in `fmBody`. Returns the byte range
 *  spanning the header line through the last indented child (and its
 *  trailing newline). Returns `null` when the section doesn't exist. */
function findPreflightBlockRange(fmBody: string): BlockRange | null {
  const lines = fmBody.split("\n");
  let cursor = 0;
  let headerStart: number | null = null;
  let blockEnd: number | null = null;

  for (let idx = 0; idx < lines.length; idx++) {
    const line = lines[idx]!;
    const lineLen = line.length;
    const lineEndCursor = cursor + lineLen;

    if (headerStart === null) {
      const trimmed = line.replace(/[\r\n]+$/, "");
      if (trimmed === "preflight:") {
        headerStart = cursor;
        // Walk forward while indented children continue.
        let inner = idx + 1;
        let innerCursor = lineEndCursor + 1;
        while (inner < lines.length) {
          const next = lines[inner]!;
          const nextTrimmed = next.replace(/[\r\n]+$/, "");
          if (nextTrimmed === "") {
            // Blank line: keep searching — block-list parsers ignore them.
            innerCursor += next.length + 1;
            inner++;
            continue;
          }
          if (!next.startsWith(" ") && !next.startsWith("\t")) {
            // Hit a top-level key — block ended at the prior line.
            break;
          }
          innerCursor += next.length + 1;
          inner++;
        }
        // `innerCursor - 1` strips the synthetic newline of the last
        // line we counted; clamp to fmBody length so trailing-no-LF
        // bodies don't overshoot.
        blockEnd = Math.min(innerCursor, fmBody.length);
        break;
      }
    }
    cursor = lineEndCursor + 1;
  }

  if (headerStart === null || blockEnd === null) return null;
  return { from: headerStart, to: blockEnd };
}
