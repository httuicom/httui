// Legacy JSON body conversion. Vaults written before the fenced
// redesign store a JSON object in the body instead of raw SQL. Convert
// the block in-place on the document: replace the body with the
// extracted query and merge connection/limit/timeout into the info
// string. Runs at most once per (blockId + body-hash) combination to
// prevent re-entry after the dispatch mutates the doc.
import { useEffect, useRef } from "react";
import { EditorView } from "@codemirror/view";

import type { DbPortalEntry } from "@/lib/codemirror/cm-db-block";
import {
  parseLegacyDbBody,
  stringifyDbFenceInfo,
  type DbBlockMetadata,
} from "@/lib/blocks/db-fence";

export function useDbLegacyMigration(
  block: DbPortalEntry["block"],
  view: EditorView,
) {
  const migratedRef = useRef<string | null>(null);
  useEffect(() => {
    if (migratedRef.current === block.body) return;
    const legacy = parseLegacyDbBody(block.body);
    if (!legacy) return;
    migratedRef.current = block.body;

    const mergedMetadata: DbBlockMetadata = { ...block.metadata };
    if (legacy.connectionId && !mergedMetadata.connection) {
      mergedMetadata.connection = legacy.connectionId;
    }
    if (legacy.limit !== undefined && mergedMetadata.limit === undefined) {
      mergedMetadata.limit = legacy.limit;
    }
    if (
      legacy.timeoutMs !== undefined &&
      mergedMetadata.timeoutMs === undefined
    ) {
      mergedMetadata.timeoutMs = legacy.timeoutMs;
    }

    const newInfoLine = "```" + stringifyDbFenceInfo(mergedMetadata);
    const openLine = view.state.doc.lineAt(block.openLineFrom);

    // Replace the open fence (to update info) AND the body (to turn JSON
    // into raw SQL), leaving fence close untouched.
    view.dispatch({
      changes: [
        {
          from: openLine.from,
          to: openLine.to,
          insert: newInfoLine,
        },
        {
          from: block.bodyFrom,
          to: block.bodyTo,
          insert: legacy.query,
        },
      ],
    });
  }, [
    block.body,
    block.bodyFrom,
    block.bodyTo,
    block.metadata,
    block.openLineFrom,
    view,
  ]);
}
