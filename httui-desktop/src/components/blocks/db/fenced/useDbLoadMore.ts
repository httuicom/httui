// Load more: append the next page of rows to the current select result.
// Uses the same query + bindings as the initial run, but with
// offset = rows already fetched. The in-flight guard is a ref (not
// state); ResultTable runs its own local loading state for the button
// spinner so this callback doesn't force a panel re-render on click.
import { useCallback, useRef } from "react";
import { EditorView } from "@codemirror/view";

import type { DbPortalEntry } from "@/lib/codemirror/cm-db-block";
import { executeDbStreamed } from "@/lib/tauri/streamedExecution";
import type { Connection } from "@/lib/tauri/connections";
import { resolveRefsToBindParams } from "@/lib/blocks/references";
import { collectBlocksAboveCM } from "@/lib/blocks/document";
import { useEnvironmentStore } from "@/stores/environment";
import {
  firstSelectResult,
  type DbResponse,
} from "@/components/blocks/db/types";

export function useDbLoadMore(opts: {
  activeConnection: Connection | null;
  block: DbPortalEntry["block"];
  blockId: string;
  view: EditorView;
  filePath: string;
  response: DbResponse | null;
  setResponse: (
    updater: (prev: DbResponse | null) => DbResponse | null,
  ) => void;
}) {
  const { activeConnection, block, blockId, view, filePath, response } = opts;
  const { setResponse } = opts;
  // Dedup guard. A ref (not state) so clicking the button does not
  // trigger a re-render of the panel — the setResponse that appends the
  // new rows is the only render needed.
  const loadingMoreRef = useRef(false);

  return useCallback(async () => {
    if (loadingMoreRef.current) return;
    const connId = activeConnection?.id;
    if (!connId || !response) return;
    const first = firstSelectResult(response);
    if (!first || !first.has_more) return;

    loadingMoreRef.current = true;
    try {
      const blocksAbove = await collectBlocksAboveCM(
        view.state.doc,
        block.from,
        filePath,
      );
      const envVars = await useEnvironmentStore.getState().getActiveVariables();
      const { sql, bindValues } = resolveRefsToBindParams(
        block.body,
        blocksAbove,
        block.from,
        envVars,
      );

      const params: Record<string, unknown> = {
        connection_id: connId,
        query: sql,
        bind_values: bindValues,
        offset: first.rows.length,
        fetch_size: block.metadata.limit ?? 100,
      };
      if (block.metadata.timeoutMs !== undefined) {
        params.timeout_ms = block.metadata.timeoutMs;
      }

      const outcome = await executeDbStreamed({
        executionId: `db_${blockId}_more_${Date.now()}`,
        params,
      });
      if (outcome.status !== "success") return;

      const next = firstSelectResult(outcome.response);
      if (!next) return;

      setResponse((prev) => {
        if (!prev) return outcome.response;
        const prevFirst = firstSelectResult(prev);
        if (!prevFirst) return outcome.response;
        const idx = prev.results.findIndex((r) => r.kind === "select");
        const mergedFirst = {
          ...prevFirst,
          rows: [...prevFirst.rows, ...next.rows],
          has_more: next.has_more,
        };
        const mergedResults = [...prev.results];
        mergedResults[idx] = mergedFirst;
        return { ...prev, results: mergedResults };
      });
    } finally {
      loadingMoreRef.current = false;
    }
    // setResponse is a stable panel-owned state setter.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    activeConnection?.id,
    block.body,
    block.from,
    block.metadata.limit,
    block.metadata.timeoutMs,
    blockId,
    filePath,
    response,
    view,
  ]);
}
