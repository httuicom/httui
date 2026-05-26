// Document-level DB block scanner.
//
// Walks runbook content for ```db-<dialect> fences and surfaces a
// flat list. The right-sidebar Schema tab uses these to gate
// visibility ("tab visible only when active runbook has DB
// blocks") and to auto-pick the most-recently-used
// connection.
//
// Pure string-in / array-out so the future CM6 hook adapts
// state.doc into the same input + the test surface stays plain JS.
// Frontmatter and non-db fenced blocks are skipped.

import { parseDbFenceInfo, type DbBlockMetadata } from "@/lib/blocks/db-fence";

const FENCE_OPEN_RE = /^```([A-Za-z][A-Za-z0-9-]*)(\s.*)?$/;
const FENCE_CLOSE_PLAIN_RE = /^```\s*$/;

export interface DocDbBlock {
  /** Full first-token language tag, e.g. `db-postgres`. */
  fence: string;
  /** Parsed metadata (alias / connection / limit / timeout / display). */
  meta: DbBlockMetadata;
  /** 1-indexed line number of the opening fence. */
  line: number;
  /** Byte offset of the opening fence's first character. */
  offset: number;
}

/** Walk `content`; emit one entry per `db-*` fenced block. Skips
 *  YAML frontmatter (between leading `---` lines) and any fence
 *  whose language tag isn't `db-<something>`. */
export function findDocDbBlocks(content: string): DocDbBlock[] {
  if (!content) return [];

  let bodyStart = 0;
  let bodyLine = 1;
  if (content.startsWith("---\n") || content.startsWith("---\r\n")) {
    let scan = content.indexOf("\n") + 1;
    let line = 2;
    while (scan < content.length) {
      const eol = content.indexOf("\n", scan);
      const lineEnd = eol === -1 ? content.length : eol;
      const lineText = content.slice(scan, lineEnd).replace(/\r$/, "");
      if (lineText === "---") {
        bodyStart = lineEnd + 1;
        bodyLine = line + 1;
        break;
      }
      scan = lineEnd + 1;
      line += 1;
    }
  }

  const out: DocDbBlock[] = [];
  let offset = bodyStart;
  let lineNo = bodyLine;
  let openFence: {
    tag: string;
    line: number;
    offset: number;
    meta: DbBlockMetadata | null;
  } | null = null;

  while (offset < content.length) {
    const eol = content.indexOf("\n", offset);
    const lineEnd = eol === -1 ? content.length : eol;
    const text = content.slice(offset, lineEnd).replace(/\r$/, "");

    if (!openFence) {
      const m = FENCE_OPEN_RE.exec(text);
      if (m) {
        const tag = m[1];
        const info = (m[2] ?? "").trim();
        // `parseDbFenceInfo` is the source of truth for what counts
        // as a DB block — accepts `db`, `db-postgres`, `db-mysql`,
        // `db-sqlite`. Other languages return null and we ignore.
        const meta = parseDbFenceInfo(`${tag} ${info}`.trim());
        openFence = { tag, line: lineNo, offset, meta };
      }
    } else if (FENCE_CLOSE_PLAIN_RE.test(text)) {
      if (openFence.meta) {
        out.push({
          fence: openFence.tag,
          meta: openFence.meta,
          line: openFence.line,
          offset: openFence.offset,
        });
      }
      openFence = null;
    }

    if (eol === -1) break;
    offset = eol + 1;
    lineNo += 1;
  }

  return out;
}

/** Quick presence check — true when at least one `db-*` block exists.
 * Used by the to gate the Schema sidebar tab. */
export function hasDbBlocks(content: string): boolean {
  return findDocDbBlocks(content).length > 0;
}

/** The connection id of the LAST `db-*` block in the document. Used
 * to auto-pick the active connection.
 *  Returns null when no db block has a `connection` token. */
export function mostRecentDbConnection(content: string): string | null {
  const blocks = findDocDbBlocks(content);
  for (let i = blocks.length - 1; i >= 0; i -= 1) {
    const conn = blocks[i].meta.connection;
    if (conn && conn.trim()) {
      return conn;
    }
  }
  return null;
}
