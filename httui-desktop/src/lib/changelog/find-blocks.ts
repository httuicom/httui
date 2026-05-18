// fenced-block walker.
//
// Given the content of a `.md` file (the after-side, since hunks
// reference new line numbers there), find every executable fenced
// block and return its [start, end] line range. The third slice
// pairs each diff hunk to the block(s) it overlaps, then classifies
// the change shape (body / info-string / assertions / captures /
// added / removed).
//
// Pure logic; no markdown library — the executable-block fence
// shapes are tightly constrained (```http / ```db-<id> / ```sh /
// ```ws / ```gql) so a focused scanner is simpler than pulling in
// a CommonMark parser.

export type BlockKind = "http" | "db" | "sh" | "ws" | "gql";

export interface FencedBlock {
  kind: BlockKind;
  /** Full info-string (e.g. `"http alias=req1 timeout=30000"`). */
  infoString: string;
  /** Alias from the info-string when present (`alias=foo` token). */
  alias: string | null;
  /** 1-indexed line of the opening fence. */
  startLine: number;
  /** 1-indexed line of the closing fence. */
  endLine: number;
}

const FENCE_REGEX = /^```(http|db-[A-Za-z0-9_-]+|sh|ws|gql)(?:\s+(.+))?$/;

export function findFencedBlocks(content: string): FencedBlock[] {
  const lines = content.split("\n");
  const blocks: FencedBlock[] = [];
  let open: {
    kind: BlockKind;
    infoString: string;
    alias: string | null;
    start: number;
  } | null = null;
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]!;
    if (open === null) {
      const m = line.match(FENCE_REGEX);
      if (m) {
        const tag = m[1]!;
        const rest = (m[2] ?? "").trim();
        const kind: BlockKind = tag.startsWith("db-")
          ? "db"
          : (tag as BlockKind);
        const infoString = tag.startsWith("db-")
          ? `${tag} ${rest}`.trim()
          : rest;
        const alias = parseAliasFromInfoString(rest);
        open = {
          kind,
          infoString,
          alias,
          start: i + 1,
        };
      }
    } else {
      // Looking for the closing fence.
      if (line.trim() === "```") {
        blocks.push({
          kind: open.kind,
          infoString: open.infoString,
          alias: open.alias,
          startLine: open.start,
          endLine: i + 1,
        });
        open = null;
      }
    }
  }
  return blocks;
}

function parseAliasFromInfoString(rest: string): string | null {
  // Tokens are space-separated `key=value`; alias appears as the
  // canonical first token of HTTP/DB/etc. info strings (per
  // CLAUDE.md "alias → timeout → display → mode" order).
  for (const token of rest.split(/\s+/u)) {
    if (token.startsWith("alias=")) {
      const v = token.slice("alias=".length).trim();
      if (v.length > 0) return v;
    }
  }
  return null;
}

/**
 * Pair a 1-indexed line number to the block that contains it. Lines
 * outside any block (between blocks or in the document body) return
 * `null`. The opening + closing fence lines themselves count as part
 * of the blockuses this to detect whether an info-string change
 * touched the fence line.
 */
export function findBlockForLine(
  blocks: ReadonlyArray<FencedBlock>,
  line: number,
): FencedBlock | null {
  for (const b of blocks) {
    if (line >= b.startLine && line <= b.endLine) return b;
  }
  return null;
}

/**
 * For a given hunk's added-line set, return the unique blocks the
 * hunk touches. A hunk that adds lines spanning two blocks (e.g.
 * deleted one + added next) appears in both blocks' classification
 * results. Empty input → empty output.
 */
export function findBlocksForHunkLines(
  blocks: ReadonlyArray<FencedBlock>,
  lines: ReadonlyArray<number>,
): FencedBlock[] {
  const seen = new Set<FencedBlock>();
  for (const line of lines) {
    const block = findBlockForLine(blocks, line);
    if (block) seen.add(block);
  }
  return Array.from(seen);
}
