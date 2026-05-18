// pure unified-diff parser.
//
// of the AI commit-changelog feature classifies block-level
// changes from `git diff --cached`. The pipeline is:
//   1. parse the unified diff text into structured hunks  ← THIS FILE
//   2. for each file, identify which fenced block each hunk falls
//      inside (by walking the after-side markdown) — carries
//   3. classify each per-block change (added / removed / modified
//      body / modified info-string / modified assertions /
//      modified captures) — carries
//
// We parse just the bits we need: file paths from `--- a/x` / `+++ b/x`,
// hunks from `@@ -oldStart,oldLines +newStart,newLines @@`, and the
// per-line +/- context. We don't build the full git diff AST (no
// binary diffs, no rename detection beyond the path strings).

export interface DiffHunk {
  oldStart: number;
  oldLines: number;
  newStart: number;
  newLines: number;
  /** 1-indexed line numbers IN THE NEW FILE that were added. */
  addedLines: number[];
  /** 1-indexed line numbers IN THE OLD FILE that were removed. */
  removedLines: number[];
  /** Raw hunk text (header + body) for downstream block-detection
   * context — follow-up walks each hunk against the
   *  after-side markdown to find the block. */
  raw: string;
}

export interface DiffFile {
  /** Path on the AFTER side (`+++ b/<path>`). For deletions this is
   *  `/dev/null` and consumers should use `oldPath` instead. */
  path: string;
  oldPath: string;
  /** Pure-add file: the OLD path was `/dev/null`. */
  isAdded: boolean;
  /** Pure-delete file: the NEW path was `/dev/null`. */
  isDeleted: boolean;
  hunks: DiffHunk[];
}

export function parseUnifiedDiff(diff: string): DiffFile[] {
  if (diff.length === 0) return [];
  const lines = diff.split("\n");
  const files: DiffFile[] = [];
  let current: DiffFile | null = null;
  let currentHunk: DiffHunk | null = null;
  let oldLineNo = 0;
  let newLineNo = 0;

  const flushHunk = () => {
    if (current && currentHunk) {
      current.hunks.push(currentHunk);
    }
    currentHunk = null;
  };
  const flushFile = () => {
    flushHunk();
    if (current) files.push(current);
    current = null;
  };

  for (const line of lines) {
    if (line.startsWith("diff --git ")) {
      flushFile();
      current = {
        path: "",
        oldPath: "",
        isAdded: false,
        isDeleted: false,
        hunks: [],
      };
    } else if (line.startsWith("--- ") && current) {
      const parsed = stripAOrB(line.slice(4));
      current.oldPath = parsed;
      current.isAdded = parsed === "/dev/null";
    } else if (line.startsWith("+++ ") && current) {
      const parsed = stripAOrB(line.slice(4));
      current.path = parsed;
      current.isDeleted = parsed === "/dev/null";
    } else if (line.startsWith("@@") && current) {
      flushHunk();
      const header = parseHunkHeader(line);
      if (!header) continue;
      currentHunk = {
        ...header,
        addedLines: [],
        removedLines: [],
        raw: line + "\n",
      };
      oldLineNo = header.oldStart;
      newLineNo = header.newStart;
    } else if (currentHunk) {
      currentHunk.raw += line + "\n";
      if (line.startsWith("+") && !line.startsWith("+++")) {
        currentHunk.addedLines.push(newLineNo);
        newLineNo += 1;
      } else if (line.startsWith("-") && !line.startsWith("---")) {
        currentHunk.removedLines.push(oldLineNo);
        oldLineNo += 1;
      } else if (line.startsWith(" ")) {
        oldLineNo += 1;
        newLineNo += 1;
      } else if (line.startsWith("\\")) {
        // "\ No newline at end of file" — skip without advancing.
      }
    }
  }
  flushFile();
  return files;
}

function stripAOrB(p: string): string {
  // Handles both `a/path` / `b/path` (the conventional prefix) and a
  // bare `/dev/null`.
  const trimmed = p.trim();
  if (trimmed === "/dev/null") return trimmed;
  if (trimmed.startsWith("a/") || trimmed.startsWith("b/")) {
    return trimmed.slice(2);
  }
  return trimmed;
}

interface HunkHeader {
  oldStart: number;
  oldLines: number;
  newStart: number;
  newLines: number;
}

function parseHunkHeader(line: string): HunkHeader | null {
  // `@@ -<oldStart>[,<oldLines>] +<newStart>[,<newLines>] @@`
  const match = line.match(/^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@/u);
  if (!match) return null;
  return {
    oldStart: parseInt(match[1]!, 10),
    oldLines: match[2] ? parseInt(match[2]!, 10) : 1,
    newStart: parseInt(match[3]!, 10),
    newLines: match[4] ? parseInt(match[4]!, 10) : 1,
  };
}

/** Convenience: pick out the `*.md` files inside `runbooks/` that the
 * changelog should consider. spec says only those count. */
export function selectRunbookMd(files: ReadonlyArray<DiffFile>): DiffFile[] {
  return files.filter((f) => {
    const path = f.isDeleted ? f.oldPath : f.path;
    return path.endsWith(".md") && path.includes("runbooks/");
  });
}
