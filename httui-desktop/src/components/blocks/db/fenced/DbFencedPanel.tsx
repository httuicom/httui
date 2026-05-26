/**
 * React panel for a db-* fenced block (stage 5 of the redesign).
 *
 * Lives outside CM6's document flow: the CM extension `cm-db-block.tsx`
 * registers three container divs per block (toolbar, result, statusbar),
 * and this component mounts React into each via `createPortal`. The
 * settings drawer uses a Chakra Portal anchored to document.body (not
 * Dialog — would trap focus away from CM6).
 *
 * Execution runs through `executeDbStreamed` from stage 3. Results are
 * persisted to the SQLite block-result cache (hashed by query + connection
 * + limit + env-snapshot placeholder) so block references
 * (`{{alias.response.col}}`) continue to work across reloads.
 */

import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { EditorView } from "@codemirror/view";

import {
  setDbBlockActions,
  setDbBlockErrors,
  type DbPortalEntry,
} from "@/lib/codemirror/cm-db-block";
import {
  parseLegacyDbBody,
  stringifyDbFenceInfo,
  type DbBlockMetadata,
} from "@/lib/blocks/db-fence";
import {
  executeDbStreamed,
  cancelBlockExecution,
} from "@/lib/tauri/streamedExecution";
import {
  firstSelectResult,
  type DbResponse,
} from "@/components/blocks/db/types";
import { computeDbCacheHash } from "@/lib/blocks/hash";
import { getBlockResult, saveBlockResult } from "@/lib/tauri/commands";
import { listConnections, type Connection } from "@/lib/tauri/connections";
import { resolveRefsToBindParams } from "@/lib/blocks/references";
import { collectBlocksAboveCM } from "@/lib/blocks/document";
import { resolveConnectionIdentifier } from "@/lib/blocks/connection-resolve";
import { describeDangerousQuery } from "@/lib/blocks/sql-mutation";
import { useEnvironmentStore } from "@/stores/environment";
import { useSchemaCacheStore } from "@/stores/schemaCache";

interface DbFencedPanelProps {
  blockId: string;
  /** Current block metadata — read from the registry each render.
   *  Passed separately from `entry` so React.memo can detect updates. */
  block: DbPortalEntry["block"];
  entry: DbPortalEntry;
  view: EditorView;
  filePath: string;
}

// ExecutionState moved to ./shared.ts
import { type ExecutionState } from "./shared";
import { DbToolbar } from "./DbToolbar";
import { DbStatusBar } from "./DbStatusBar";
import { DbResult } from "./DbResultTabs";
import { DbSettingsDrawer as DbDrawer } from "./DbSettingsDrawer";
import { ConfirmRunDialog } from "./ConfirmRunDialog";

// ───── Main panel ─────

export const DbFencedPanel = memo(function DbFencedPanel({
  blockId,
  block,
  entry,
  view,
  filePath,
}: DbFencedPanelProps) {
  const [executionState, setExecutionState] = useState<ExecutionState>("idle");
  const [response, setResponse] = useState<DbResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [durationMs, setDurationMs] = useState<number | null>(null);
  const [cached, setCached] = useState(false);
  const [connections, setConnections] = useState<Connection[]>([]);
  const [drawerOpen, setDrawerOpen] = useState(false);
  /** When set, blocks execution behind a user confirmation. The stored
   *  `continueRun` callback runs the query for real if the user accepts. */
  const [pendingConfirm, setPendingConfirm] = useState<{
    reason: string;
    continueRun: () => void;
  } | null>(null);
  /** Milliseconds elapsed since the current run started; drives the live
   *  timer shown in the result panel during execution. Reset to 0 when
   *  not running. */
  const [liveElapsedMs, setLiveElapsedMs] = useState(0);
  /**
   * Last-execution bindings: `{{ref.raw}} → resolved value`. Shown in the
   * drawer's Resolved bindings panel so users can debug what the driver
   * actually received.
   */
  const [resolvedBindings, setResolvedBindings] = useState<
    { placeholder: string; raw: string; value: unknown }[]
  >([]);
  // Load-more dedup guard. A ref (not state) so clicking the button does
  // not trigger a re-render of the panel — the setResponse that appends
  // the new rows is the only render needed.
  const loadingMoreRef = useRef(false);
  const abortRef = useRef<AbortController | null>(null);

  const activeConnection = useMemo(
    () => resolveConnectionIdentifier(connections, block.metadata.connection),
    [connections, block.metadata.connection],
  );

  // Load connections once
  useEffect(() => {
    listConnections()
      .then(setConnections)
      .catch(() => {});
  }, []);

  // Warm the schema cache so SQL autocomplete (tables/columns) is ready
  // without the user having to wait for the first keystroke to fire.
  useEffect(() => {
    if (!activeConnection?.id) return;
    void useSchemaCacheStore.getState().ensureLoaded(activeConnection.id);
  }, [activeConnection?.id]);

  // SQL error squiggle: paint red wavy underline at the line/col reported
  // by the backend. Triggered whenever the response changes; cleared when
  // the user starts editing the query again (new body → stale location).
  useEffect(() => {
    const errorMarks: {
      line: number;
      column: number;
      message?: string;
    }[] = [];
    for (const result of response?.results ?? []) {
      if (
        result.kind === "error" &&
        typeof result.line === "number" &&
        typeof result.column === "number" &&
        result.line > 0 &&
        result.column > 0
      ) {
        errorMarks.push({
          line: result.line,
          column: result.column,
          message: result.message,
        });
      }
    }
    setDbBlockErrors(view, blockId, errorMarks);
  }, [response, blockId, view]);

  // Clear any lingering squiggle when the user edits the body — the old
  // position no longer points at the same token.
  useEffect(() => {
    setDbBlockErrors(view, blockId, []);
  }, [block.body, blockId, view]);

  // Live elapsed timer for running state. Ticks every 100ms; stops when
  // the execution leaves the running state. Cheap since only one block
  // can be running per panel instance.
  useEffect(() => {
    if (executionState !== "running") {
      setLiveElapsedMs(0);
      return;
    }
    const startedAt = performance.now();
    const id = window.setInterval(() => {
      setLiveElapsedMs(Math.round(performance.now() - startedAt));
    }, 100);
    return () => {
      window.clearInterval(id);
    };
  }, [executionState]);

  // ── Legacy JSON body conversion ──
  // Vaults written before stage 4 store a JSON object in the body instead
  // of raw SQL. Convert the block in-place on the document: replace the
  // body with the extracted query and merge connection/limit/timeout into
  // the info string. This runs at most once per (blockId + body-hash)
  // combination to prevent re-entry after the dispatch mutates the doc.
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

  // Load cached result on mount / when block body + connection change
  useEffect(() => {
    if (!filePath) return;
    const connId = activeConnection?.id ?? block.metadata.connection ?? "";
    if (!connId || !block.body.trim()) return;

    let cancelled = false;
    (async () => {
      try {
        const envVars = await useEnvironmentStore
          .getState()
          .getActiveVariables();
        const hash = await computeDbCacheHash(block.body, connId, envVars);
        const row = await getBlockResult(filePath, hash);
        if (cancelled || !row) return;
        const parsed = JSON.parse(row.response) as DbResponse;
        setResponse(parsed);
        setDurationMs(row.elapsed_ms ?? null);
        setCached(true);
        if (row.status === "success") setExecutionState("success");
        else setExecutionState("error");
      } catch {
        // Cache miss or corrupt — stay idle.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [filePath, block.body, activeConnection?.id, block.metadata.connection]);

  // ── Execution ──
  // Internal: actually dispatches the backend call. `runBlock` (below)
  // applies read-only / unscoped-mutation gating before calling this.
  const executeRun = useCallback(async () => {
    if (executionState === "running") return;
    const connId = activeConnection?.id;
    if (!connId) {
      setError("No connection selected — open settings and pick one.");
      setExecutionState("error");
      return;
    }
    if (!block.body.trim()) {
      setError("Query is empty.");
      setExecutionState("error");
      return;
    }

    setError(null);
    setCached(false);
    setExecutionState("running");
    const abort = new AbortController();
    abortRef.current = abort;

    const executionId = `db_${blockId}_${Date.now()}`;
    const startedAt = performance.now();

    try {
      // ── Resolve {{ref}} references into bind params ──
      // Collect blocks above (for {{alias.response.col}} resolution) and
      // the active environment's variables (for {{ENV_KEY}}).
      const blocksAbove = await collectBlocksAboveCM(
        view.state.doc,
        block.from,
        filePath,
      );
      const envVars = await useEnvironmentStore.getState().getActiveVariables();

      const {
        sql,
        bindValues,
        errors: refErrors,
      } = resolveRefsToBindParams(block.body, blocksAbove, block.from, envVars);
      if (refErrors.length > 0) {
        setError(`Reference errors:\n${refErrors.join("\n")}`);
        setExecutionState("error");
        return;
      }

      // Capture the resolved mapping so the drawer can display it.
      const rawRefs = Array.from(block.body.matchAll(/\{\{([^}]+)\}\}/g));
      const bindingsForDrawer = rawRefs.map((m, i) => ({
        placeholder: `$${i + 1}`,
        raw: m[0],
        value: bindValues[i],
      }));
      setResolvedBindings(bindingsForDrawer);

      const params: Record<string, unknown> = {
        connection_id: connId,
        query: sql,
        bind_values: bindValues,
        offset: 0,
        fetch_size: block.metadata.limit ?? 100,
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

      setResponse(outcome.response);
      setDurationMs(outcome.response.stats.elapsed_ms || elapsed);
      setExecutionState("success");

      // Persist to cache. Hash key includes env snapshot so different
      // environments don't share cache entries for the same query.
      try {
        const hash = await computeDbCacheHash(block.body, connId, envVars);
        const sel = firstSelectResult(outcome.response);
        await saveBlockResult(
          filePath,
          hash,
          "success",
          JSON.stringify(outcome.response),
          outcome.response.stats.elapsed_ms || elapsed,
          sel ? sel.rows.length : null,
        );
      } catch {
        // Cache write is best-effort.
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setExecutionState("error");
    } finally {
      abortRef.current = null;
    }
  }, [
    activeConnection?.id,
    block.body,
    block.from,
    block.metadata.limit,
    block.metadata.timeoutMs,
    blockId,
    executionState,
    filePath,
    view,
  ]);

  /**
   * Guard the execute with a confirmation prompt when the query is a
   * mutation on a read-only connection, or an UPDATE/DELETE with no
   * WHERE. The prompt UI is a Portal + Box rendered below.
   */
  const runBlock = useCallback(() => {
    const reason = describeDangerousQuery(
      block.body,
      activeConnection?.is_readonly ?? false,
    );
    if (reason) {
      setPendingConfirm({
        reason,
        continueRun: () => {
          setPendingConfirm(null);
          void executeRun();
        },
      });
      return;
    }
    void executeRun();
  }, [block.body, activeConnection?.is_readonly, executeRun]);

  const cancelBlock = useCallback(() => {
    const abort = abortRef.current;
    if (abort) {
      abort.abort();
      abortRef.current = null;
    }
    // Best-effort: also tell the backend (in case abort raced).
    void cancelBlockExecution(`db_${blockId}`);
  }, [blockId]);

  // ── EXPLAIN ──
  // Runs ONLY the first SQL statement wrapped in an EXPLAIN prefix, as a
  // one-off (no cache), and folds the plan rows into the current
  // response's `plan` field so the Plan tab lights up.
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
  const runExplain = useCallback(async () => {
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
  }, [
    activeConnection?.id,
    activeConnection?.driver,
    block.body,
    block.from,
    block.metadata.dialect,
    block.metadata.timeoutMs,
    blockId,
    executionState,
    filePath,
    view,
  ]);

  // ── Load more: append the next page of rows to the current select
  // result. Uses the same query + bindings as the initial run, but with
  // offset = rows already fetched. The in-flight guard is a ref (not
  // state); ResultTable runs its own local loading state for the button
  // spinner so this callback doesn't force a panel re-render on click.
  const loadMore = useCallback(async () => {
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

  // Register actions with the registry so ⌘↵ / ⌘. / ⌘⇧E can dispatch
  useEffect(() => {
    setDbBlockActions(blockId, {
      onRun: runBlock,
      onCancel: cancelBlock,
      onOpenSettings: () => setDrawerOpen(true),
      onExplain: runExplain,
    });
  }, [blockId, runBlock, cancelBlock, runExplain]);

  // ── Info-string editing (drawer) ──
  const updateMetadata = useCallback(
    (patch: Partial<DbBlockMetadata>) => {
      const next: DbBlockMetadata = { ...block.metadata, ...patch };
      // Re-stringify and dispatch a change that rewrites only the info string
      // portion of the open fence line.
      const infoText = stringifyDbFenceInfo(next);
      const openLine = view.state.doc.lineAt(block.openLineFrom);
      view.dispatch({
        changes: {
          from: openLine.from,
          to: openLine.to,
          insert: "```" + infoText,
        },
      });
    },
    [block.metadata, block.openLineFrom, view],
  );

  const deleteBlockFromDoc = useCallback(() => {
    // Remove the entire block range plus its trailing newline (if any) so
    // we don't leave a blank line in its place.
    const from = block.from;
    const to = Math.min(block.to + 1, view.state.doc.length);
    view.dispatch({ changes: { from, to, insert: "" } });
    setDrawerOpen(false);
  }, [block.from, block.to, view]);

  // ── Portals ──

  const toolbarNode = entry.toolbar;
  const resultNode = entry.result;
  const statusbarNode = entry.statusbar;

  return (
    <>
      {toolbarNode &&
        createPortal(
          <DbToolbar
            metadata={block.metadata}
            activeConnection={activeConnection}
            executionState={executionState}
            onRun={runBlock}
            onCancel={cancelBlock}
            onExplain={runExplain}
            onOpenSettings={() => setDrawerOpen(true)}
          />,
          toolbarNode,
        )}

      {resultNode &&
        createPortal(
          <DbResult
            executionState={executionState}
            response={response}
            error={error}
            cached={cached}
            liveElapsedMs={liveElapsedMs}
            connection={activeConnection?.name ?? block.metadata.connection}
            onCancel={cancelBlock}
            onLoadMore={loadMore}
          />,
          resultNode,
        )}

      {statusbarNode &&
        createPortal(
          <DbStatusBar
            connection={activeConnection?.name ?? block.metadata.connection}
            isReadonly={activeConnection?.is_readonly ?? false}
            hasActiveConnection={!!activeConnection}
            durationMs={durationMs}
            executionState={executionState}
            response={response}
            cached={cached}
            query={block.body}
            alias={block.metadata.alias}
          />,
          statusbarNode,
        )}

      {drawerOpen && (
        <DbDrawer
          metadata={block.metadata}
          connections={connections}
          activeConnection={activeConnection}
          resolvedBindings={resolvedBindings}
          onClose={() => setDrawerOpen(false)}
          onUpdate={updateMetadata}
          onDelete={deleteBlockFromDoc}
          onConnectionsChanged={setConnections}
        />
      )}

      {pendingConfirm && (
        <ConfirmRunDialog
          reason={pendingConfirm.reason}
          onCancel={() => setPendingConfirm(null)}
          onConfirm={pendingConfirm.continueRun}
        />
      )}
    </>
  );
});

// DbStatusBar moved to ./DbStatusBar.tsx
