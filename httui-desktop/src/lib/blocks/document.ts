import type { Text as CMText } from "@codemirror/state";
import type { BlockContext } from "./references";
import { getBlockResult } from "@/lib/tauri/commands";
import { hashBlockContent, computeDbCacheHash } from "./hash";
import { findFencedBlocks } from "@/lib/codemirror/cm-block-widgets";
import { findDbBlocks } from "@/lib/codemirror/cm-db-block";
import {
  extractAlias,
  langToBlockType,
} from "@/lib/codemirror/block-widget-context";
import { resolveConnectionIdentifier } from "./connection-resolve";
import { listConnections, type Connection } from "@/lib/tauri/connections";
import { useEnvironmentStore } from "@/stores/environment";

const EXECUTABLE_LANGS = ["http", "db"];

// ---------------------------------------------------------------------------
// CodeMirror 6 — walks CM Text (markdown) to find executable fenced blocks.
// (The TipTap variants `collectBlocksAbove`/`collectAllBlocks` were removed
// when TipTap was retired; the CM6 versions below are the only callers
// across the app.)
// ---------------------------------------------------------------------------

/** Check if a fenced block language tag is an executable block type */
function isExecutableLang(lang: string): boolean {
  if (EXECUTABLE_LANGS.includes(lang)) return true;
  if (lang.startsWith("db-")) return true;
  return false;
}

/**
 * Internal shape: a `BlockContext` plus the raw `connection=…` identifier
 * from the db fence info. Only DB blocks carry this. It's threaded from the
 * block scanner through `populateCachedResults` so we can resolve the
 * connection name/UUID to a real connection and replicate the DB block's
 * cache hash formula.
 */
interface CollectedBlock extends BlockContext {
  _dbConnectionIdentifier?: string;
}

/**
 * Populate cached results for a list of collected blocks. For http/e2e
 * blocks the cache key is just `hashBlockContent(content)` (same as what
 * the write side computes). For DB blocks the key is `computeDbCacheHash`
 * which folds in the resolved connection UUID and a snapshot of the env
 * vars referenced by the query — both sides MUST match or the lookup
 * silently misses every cached row.
 */
async function populateCachedResults(
  blocks: CollectedBlock[],
  filePath: string,
): Promise<void> {
  const hasDb = blocks.some((b) => b.blockType === "db");
  let connections: Connection[] = [];
  let envVars: Record<string, string> = {};
  if (hasDb) {
    try {
      [connections, envVars] = await Promise.all([
        listConnections(),
        useEnvironmentStore.getState().getActiveVariables(),
      ]);
    } catch {
      // Best-effort: if either fetch fails, DB lookups below just miss.
    }
  }

  await Promise.all(
    blocks.map(async (block) => {
      if (!block.content) return;
      try {
        let hash: string;
        if (block.blockType === "db") {
          const conn = resolveConnectionIdentifier(
            connections,
            block._dbConnectionIdentifier,
          );
          if (!conn) return;
          hash = await computeDbCacheHash(block.content, conn.id, envVars);
        } else {
          hash = await hashBlockContent(block.content);
        }
        const cached = await getBlockResult(filePath, hash);
        if (cached) {
          block.cachedResult = {
            status: cached.status,
            response: cached.response,
          };
        }
      } catch {
        // Cache lookup failed, leave as null
      }
    }),
  );
}

/**
 * Unified scan over every executable fenced block (http, e2e, db, db-*).
 *
 * `findFencedBlocks` only emits http/e2e because the editor widget system
 * renders db blocks through a dedicated extension (`cm-db-block.tsx`) and
 * would otherwise try to mount a second widget on top of them. For the
 * dependency graph we need both, so we merge the two scanners and sort by
 * document position to preserve the "blocks above" semantics.
 */
function collectFencedBlockContexts(doc: CMText): CollectedBlock[] {
  const contexts: CollectedBlock[] = [];

  for (const fb of findFencedBlocks(doc)) {
    if (!isExecutableLang(fb.lang)) continue;
    const alias = extractAlias(fb.info);
    if (!alias) continue;
    contexts.push({
      alias,
      blockType: langToBlockType(fb.lang),
      pos: fb.from,
      content: fb.content,
      cachedResult: null,
    });
  }

  for (const db of findDbBlocks(doc)) {
    const alias = extractAlias(db.info);
    if (!alias) continue;
    contexts.push({
      alias,
      blockType: langToBlockType(db.lang),
      pos: db.from,
      content: db.body,
      cachedResult: null,
      _dbConnectionIdentifier: db.metadata.connection,
    });
  }

  contexts.sort((a, b) => a.pos - b.pos);
  return contexts;
}

/**
 * Collect all executable blocks above a given position in a CM6 document.
 * CM6 equivalent of collectBlocksAbove (TipTap version).
 */
export async function collectBlocksAboveCM(
  doc: CMText,
  beforePos: number,
  filePath: string,
): Promise<BlockContext[]> {
  const blocks = collectFencedBlockContexts(doc).filter(
    (b) => b.pos < beforePos,
  );
  await populateCachedResults(blocks, filePath);
  return blocks;
}

/**
 * Collect ALL executable blocks in a CM6 document (for dependency resolution).
 * CM6 equivalent of collectAllBlocks (TipTap version).
 */
export async function collectAllBlocksCM(
  doc: CMText,
  filePath: string,
): Promise<BlockContext[]> {
  const blocks = collectFencedBlockContexts(doc);
  await populateCachedResults(blocks, filePath);
  return blocks;
}
