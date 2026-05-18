// per-block change classifier.
//
// Combines `parseUnifiedDiff` (slice 1) + `findFencedBlocks`
// (slice 2) to classify every block-level change in a touched
// `*.md` file as one of:
//
//   - Added                — a fenced block exists in the after-side
//     content but not the before-side (matched by alias OR by
//     position when alias is null)
//   - Removed              — exists in before but not after
//   - ModifiedBody         — body lines (between fences, excluding
//     `# expect:` / `# capture:` marker sections) changed
//   - ModifiedInfoString   — only the opening fence line changed
//   - ModifiedAssertions   — `# expect:` section diff
//   - ModifiedCaptures     — `# capture:` section diff
//
// A single hunk can produce multiple `BlockChange` entries when it
// crosses block boundaries (rare but possible). The classifier
// deduplicates `(blockKey, kind)` so a hunk that touches the same
// block twice doesn't double-count.

import { type DiffHunk } from "./parse-diff";
import {
  type BlockKind,
  type FencedBlock,
  findBlockForLine,
  findFencedBlocks,
} from "./find-blocks";

export type ChangeKind =
  | "added"
  | "removed"
  | "modified-body"
  | "modified-info-string"
  | "modified-assertions"
  | "modified-captures";

export interface BlockChange {
  kind: ChangeKind;
  blockKind: BlockKind;
  /** Block alias when present, else null (positional-id fallback
   *  is the consumer's job — slice 3 doesn't synthesize bNN ids). */
  alias: string | null;
  /** Info-string of the after-side block (or before-side for
   *  Removed). Useful to render "GET /users" in the AI prompt. */
  infoString: string;
}

export interface ClassifyInput {
  beforeContent: string;
  afterContent: string;
  hunks: ReadonlyArray<DiffHunk>;
}

export function classifyBlockChanges(input: ClassifyInput): BlockChange[] {
  const beforeBlocks = findFencedBlocks(input.beforeContent);
  const afterBlocks = findFencedBlocks(input.afterContent);
  const beforeKeys = blockKeys(beforeBlocks);
  const afterKeys = blockKeys(afterBlocks);

  const out: BlockChange[] = [];
  const seen = new Set<string>();
  const push = (key: string, change: BlockChange) => {
    const dedupKey = `${key}::${change.kind}`;
    if (seen.has(dedupKey)) return;
    seen.add(dedupKey);
    out.push(change);
  };

  // Added: in after but not before.
  for (const b of afterBlocks) {
    const key = blockKey(b);
    if (!beforeKeys.has(key)) {
      push(key, {
        kind: "added",
        blockKind: b.kind,
        alias: b.alias,
        infoString: b.infoString,
      });
    }
  }

  // Removed: in before but not after.
  for (const b of beforeBlocks) {
    const key = blockKey(b);
    if (!afterKeys.has(key)) {
      push(key, {
        kind: "removed",
        blockKind: b.kind,
        alias: b.alias,
        infoString: b.infoString,
      });
    }
  }

  // Modified: per-hunk classification against the after-side
  // structure. Skip hunks whose touched lines fall on already-
  // added blocks (those are covered by the "added" pass).
  const beforeKeySet = beforeKeys;
  const afterKeySet = afterKeys;
  for (const hunk of input.hunks) {
    for (const line of hunk.addedLines) {
      const block = findBlockForLine(afterBlocks, line);
      if (!block) continue;
      const key = blockKey(block);
      if (!beforeKeySet.has(key)) continue; // Added block — already pushed.
      const subKind = classifyHunkLineWithinBlock(
        block,
        line,
        input.afterContent,
      );
      push(key, {
        kind: subKind,
        blockKind: block.kind,
        alias: block.alias,
        infoString: block.infoString,
      });
    }
    for (const line of hunk.removedLines) {
      const block = findBlockForLine(beforeBlocks, line);
      if (!block) continue;
      const key = blockKey(block);
      if (!afterKeySet.has(key)) continue; // Removed block — already pushed.
      const subKind = classifyHunkLineWithinBlock(
        block,
        line,
        input.beforeContent,
      );
      push(key, {
        kind: subKind,
        blockKind: block.kind,
        alias: block.alias,
        infoString: block.infoString,
      });
    }
  }

  return out;
}

/** Stable identifier for matching before↔after blocks. Uses alias
 *  when present (canonical), else falls back to "kind@startLine"
 *  positional matching — imperfect across moves but good enough for
 *  the slice-1 classifier. */
function blockKey(b: FencedBlock): string {
  if (b.alias) return `alias:${b.alias}`;
  return `pos:${b.kind}@${b.startLine}`;
}

function blockKeys(bs: ReadonlyArray<FencedBlock>): Set<string> {
  const set = new Set<string>();
  for (const b of bs) set.add(blockKey(b));
  return set;
}

function classifyHunkLineWithinBlock(
  block: FencedBlock,
  line: number,
  content: string,
): ChangeKind {
  if (line === block.startLine) return "modified-info-string";
  // Find the source of `line` within the content; check if the line
  // text starts a marker section. Marker sections persist for the
  // remainder of the block, so we walk back to find the most recent
  // marker line.
  const lines = content.split("\n");
  // 1-indexed → 0-indexed for array access.
  const cur = lines[line - 1] ?? "";
  if (isAssertionMarker(cur)) return "modified-assertions";
  if (isCaptureMarker(cur)) return "modified-captures";
  // Walk back within the block looking for the most recent marker.
  for (let i = line - 1; i >= block.startLine; i--) {
    const text = lines[i - 1] ?? "";
    if (isAssertionMarker(text)) return "modified-assertions";
    if (isCaptureMarker(text)) return "modified-captures";
  }
  return "modified-body";
}

function isAssertionMarker(line: string): boolean {
  return /^#\s*expect\s*:/iu.test(line.trim());
}

function isCaptureMarker(line: string): boolean {
  return /^#\s*capture\s*:/iu.test(line.trim());
}
