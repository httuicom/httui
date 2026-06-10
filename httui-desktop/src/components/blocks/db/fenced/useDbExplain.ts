// EXPLAIN runner. Runs ONLY the first SQL statement wrapped in an
// EXPLAIN prefix, as a one-off (no cache), and folds the plan rows into
// the current response's `plan` field so the Plan tab lights up.
//
// Why only the first statement: backend splits on `;` and treats each
// chunk as its own driver call. If we prefix the whole body with
// `EXPLAIN ` only the first chunk is explained — the rest run for real.
// That's a footgun on multi-statement bodies, so we drop everything
// after the first `;` for the EXPLAIN run only. Body is unchanged.
//
// ANALYZE is intentionally omitted: in Postgres it executes the query
// for real, which would make clicking ▦ on an UPDATE/DELETE a latent
// footgun. The non-analyze plan is enough for 90% of debugging.
import { useCallback, type MutableRefObject } from "react";
import { EditorView } from "@codemirror/view";

import type { DbPortalEntry } from "@/lib/codemirror/cm-db-block";
import { executeDbStreamed } from "@/lib/tauri/streamedExecution";
import type { Connection } from "@/lib/tauri/connections";
import { resolveRefsToBindParams } from "@/lib/blocks/references";
import { collectBlocksAboveCM } from "@/lib/blocks/document";
import { useEnvironmentStore } from "@/stores/environment";
import type { DbResponse } from "@/components/blocks/db/types";
import { type ExecutionState } from "./shared";

export interface DbRunStateSetters {
  setExecutionState: (s: ExecutionState) => void;
  setError: (e: string | null) => void;
  setDurationMs: (ms: number | null) => void;
  setResponse: (
    updater: (prev: DbResponse | null) => DbResponse | null,
  ) => void;
  setCached: (c: boolean) => void;
}

export function useDbExplain(opts: {
  executionState: ExecutionState;
  activeConnection: Connection | null;
  block: DbPortalEntry["block"];
  blockId: string;
  view: EditorView;
  filePath: string;
  abortRef: MutableRefObject<AbortController | null>;
  setters: DbRunStateSetters;
}) {
  const {
    executionState,
    activeConnection,
    block,
    blockId,
    view,
    filePath,
    abortRef,
    setters,
  } = opts;
  const { setExecutionState, setError, setDurationMs, setResponse, setCached } =
    setters;

  return useCallback(async () => {
    if (executionState === "running") return;
    const connId = activeConnection?.id;
    if (!connId) return;
    const body = block.body.trim();
    if (!body) return;

    // First non-empty statement only. Naive `;` split is good enough for
    // EXPLAIN — strings/identifiers containing `;` are vanishingly rare in
    // a query someone is debugging.
    const firstStatement =
      body
        .split(";")
        .map((s) => s.trim())
        .find((s) => s.length > 0) ?? body;

    // Pick the EXPLAIN flavour from the actual driver, falling back to
    // the fence dialect, then to plain `EXPLAIN`. The fence dialect alone
    // is unreliable: a `db` (generic) fence pointing at a SQLite
    // connection would otherwise emit `EXPLAIN <sql>` and return raw VDBE
    // bytecode, which is useless for 99% of users debugging a query.
    // For SQLite we want `EXPLAIN QUERY PLAN` (SCAN/SEARCH/USING INDEX).
    // Postgres/MySQL plain `EXPLAIN` returns the human plan already.
    const dialect = block.metadata.dialect;
    const driver = activeConnection?.driver;
    const isSqlite = driver === "sqlite" || dialect === "sqlite";
    const prefix = isSqlite ? "EXPLAIN QUERY PLAN " : "EXPLAIN ";
    // Skip wrapping if the user already typed EXPLAIN themselves — double
    // EXPLAIN is a syntax error everywhere.
    const alreadyExplain = /^\s*EXPLAIN\b/i.test(firstStatement);

    setError(null);
    setExecutionState("running");
    const abort = new AbortController();
    abortRef.current = abort;
    const executionId = `db_${blockId}_explain_${Date.now()}`;
    const startedAt = performance.now();

    try {
      const blocksAbove = await collectBlocksAboveCM(
        view.state.doc,
        block.from,
        filePath,
      );
      const envVars = await useEnvironmentStore.getState().getActiveVariables();
      const {
        sql: resolvedBody,
        bindValues,
        errors: refErrors,
      } = resolveRefsToBindParams(
        firstStatement,
        blocksAbove,
        block.from,
        envVars,
      );
      if (refErrors.length > 0) {
        setError(`Reference errors:\n${refErrors.join("\n")}`);
        setExecutionState("error");
        return;
      }
      const finalSql = alreadyExplain ? resolvedBody : prefix + resolvedBody;

      const params: Record<string, unknown> = {
        connection_id: connId,
        query: finalSql,
        bind_values: bindValues,
        offset: 0,
        fetch_size: 1000,
      };
      if (block.metadata.timeoutMs !== undefined) {
        params.timeout_ms = block.metadata.timeoutMs;
      }

      const outcome = await executeDbStreamed({
        executionId,
        params,
        signal: abort.signal,
      });
      const elapsed = Math.round(performance.now() - startedAt);

      if (outcome.status === "cancelled") {
        setExecutionState("cancelled");
        setDurationMs(elapsed);
        return;
      }
      if (outcome.status === "error") {
        setError(outcome.message);
        setExecutionState("error");
        setDurationMs(elapsed);
        return;
      }

      // Surface SQL-level errors (kind: "error" inside a result) the same
      // way as a regular run — they belong in the error state, not stuffed
      // into the Plan tab as JSON noise.
      const firstResult = outcome.response.results[0];
      if (firstResult && firstResult.kind === "error") {
        setError(firstResult.message);
        setExecutionState("error");
        setDurationMs(outcome.response.stats.elapsed_ms || elapsed);
        return;
      }

      // Only populate plan when we actually have plan rows to show.
      // SELECT result with rows = real plan output (sqlite EXPLAIN returns
      // bytecode rows, postgres EXPLAIN returns a single text-column
      // table, etc — all selectable). Anything else (mutation? empty?)
      // means the driver didn't return a plan; fall through to "no plan".
      const explainResult =
        firstResult && firstResult.kind === "select" ? firstResult : null;
      if (!explainResult || explainResult.rows.length === 0) {
        setError(
          "EXPLAIN didn't return a plan — the driver may not support EXPLAIN for this query.",
        );
        setExecutionState("error");
        setDurationMs(outcome.response.stats.elapsed_ms || elapsed);
        return;
      }

      setResponse((prev) => {
        const base: DbResponse = prev ?? {
          results: [],
          messages: [],
          stats: { elapsed_ms: 0 },
        };
        return {
          ...base,
          plan: explainResult.rows,
          stats: {
            ...base.stats,
            elapsed_ms: outcome.response.stats.elapsed_ms || elapsed,
          },
        };
      });
      setDurationMs(outcome.response.stats.elapsed_ms || elapsed);
      setCached(false);
      setExecutionState("success");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setExecutionState("error");
    } finally {
      abortRef.current = null;
    }
    // Setters are stable panel-owned state setters.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    executionState,
    activeConnection?.id,
    activeConnection?.driver,
    block.body,
    block.from,
    block.metadata.dialect,
    block.metadata.timeoutMs,
    blockId,
    filePath,
    view,
  ]);
}
