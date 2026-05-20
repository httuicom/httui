/**
 * History + examples loading and the drawer-side action handlers for
 * an HTTP block. Extracted from `HttpFencedPanel.tsx` during the
 * follow-up to A1+A2a so the orchestrator stays < 600 L.
 *
 * Owns:
 *  - `historyEntries` / `examples` state + their per-(file, alias)
 *    SQLite loads, gated on `drawerOpen` so the queries don't fire
 *    while the drawer is hidden.
 *  - Refresh ticks bumped after a successful insert / delete so the
 *    loaders re-run without coupling deps to fast-changing array refs.
 *  - `recordHistory` (called by the run pipeline's onOutcome) +
 *    `purgeHistory` / `saveExample` / `restoreExample` /
 *    `deleteExample` (called by the drawer JSX).
 *
 * `restoreExample` needs to push a result into the FSM owned by the
 * panel; it receives `applyCachedResult` + `setLastRunAt` + a
 * `closeDrawer` callback via params.
 */

import { useCallback, useEffect, useState } from "react";

import {
  deleteBlockExample,
  insertBlockHistory,
  listBlockExamples,
  listBlockHistory,
  purgeBlockHistory,
  saveBlockExample,
  type BlockExample,
  type HistoryEntry,
  type HttpBlockSettings,
} from "@/lib/tauri/commands";
import type { HttpResponseFull } from "@/lib/tauri/streamedExecution";

interface RecordHistoryInfo {
  method: string;
  url: string;
  status: number | null;
  requestSize: number | null;
  responseSize: number | null;
  elapsedMs: number;
  outcome: "success" | "error" | "cancelled";
}

interface UseHttpDrawerDataParams {
  filePath: string;
  alias: string | undefined;
  drawerOpen: boolean;
  settings: HttpBlockSettings;
  applyCachedResult: (response: HttpResponseFull, elapsed?: number) => void;
  setLastRunAt: (d: Date | null) => void;
  closeDrawer: () => void;
}

interface UseHttpDrawerDataResult {
  historyEntries: HistoryEntry[];
  examples: BlockExample[];
  /** Called by the run pipeline (`onOutcome`) after every outcome. */
  recordHistory: (info: RecordHistoryInfo) => Promise<void>;
  /** Bump the history-refresh tick — also useful on drawer-open. */
  bumpHistoryTick: () => void;
  /** Drawer action: clear all history for this (file, alias). */
  purgeHistory: () => Promise<void>;
  /** Drawer action: snapshot the current response as a named example. */
  saveExample: (name: string, response: HttpResponseFull) => Promise<void>;
  /** Drawer action: restore an example into the FSM + close the drawer. */
  restoreExample: (ex: BlockExample) => void;
  /** Drawer action: delete one example by id. */
  deleteExample: (id: number) => Promise<void>;
}

export function useHttpDrawerData({
  filePath,
  alias,
  drawerOpen,
  settings,
  applyCachedResult,
  setLastRunAt,
  closeDrawer,
}: UseHttpDrawerDataParams): UseHttpDrawerDataResult {
  const [historyEntries, setHistoryEntries] = useState<HistoryEntry[]>([]);
  const [examples, setExamples] = useState<BlockExample[]>([]);
  // Ticks bumped on every successful insert + on drawer-open so the
  // loaders re-fetch without coupling their `useEffect` deps to a
  // fast-changing array reference.
  const [historyRefreshTick, setHistoryRefreshTick] = useState(0);
  const [examplesRefreshTick, setExamplesRefreshTick] = useState(0);

  /** Persist a row in `block_run_history`. Best-effort: a write
   *  failure doesn't block the user from seeing the response. */
  const recordHistory = useCallback(
    async (info: RecordHistoryInfo) => {
      if (!alias) return; // No alias → no stable key to bucket history under.
      // User opt-out (Onda 1) — drawer toggle persisted in `block_settings`.
      if (settings.historyDisabled === true) return;
      try {
        await insertBlockHistory({
          file_path: filePath,
          block_alias: alias,
          method: info.method,
          url_canonical: info.url,
          status: info.status,
          request_size: info.requestSize,
          response_size: info.responseSize,
          elapsed_ms: info.elapsedMs,
          outcome: info.outcome,
        });
        setHistoryRefreshTick((t) => t + 1);
      } catch {
        /* Best-effort. */
      }
    },
    [alias, filePath, settings.historyDisabled],
  );

  // History list — loaded on drawer-open + refresh ticks. The
  // `if (!alias)` branch resets state so a block with a stale alias
  // (from a previous load) doesn't show those rows when the block's
  // alias is cleared. The reset is necessary for UI coherence; one
  // synchronous setState per alias-change is bounded — not the
  // cascading pattern the rule guards against.
  useEffect(() => {
    if (!drawerOpen) return;
    if (!alias) {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setHistoryEntries([]);
      return;
    }
    let cancelled = false;
    void (async () => {
      try {
        const rows = await listBlockHistory(filePath, alias);
        if (!cancelled) setHistoryEntries(rows);
      } catch {
        if (!cancelled) setHistoryEntries([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [drawerOpen, filePath, alias, historyRefreshTick]);

  // Examples list — loaded on drawer-open + refresh ticks. Same
  // alias-change reset rationale as the history effect above.
  useEffect(() => {
    if (!drawerOpen) return;
    if (!alias) {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setExamples([]);
      return;
    }
    let cancelled = false;
    void (async () => {
      try {
        const rows = await listBlockExamples(filePath, alias);
        if (!cancelled) setExamples(rows);
      } catch {
        if (!cancelled) setExamples([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [drawerOpen, filePath, alias, examplesRefreshTick]);

  const bumpHistoryTick = useCallback(
    () => setHistoryRefreshTick((t) => t + 1),
    [],
  );

  const purgeHistory = useCallback(async () => {
    if (!alias) return;
    try {
      await purgeBlockHistory(filePath, alias);
      setHistoryRefreshTick((t) => t + 1);
    } catch {
      /* Best-effort. */
    }
  }, [alias, filePath]);

  const saveExample = useCallback(
    async (name: string, response: HttpResponseFull) => {
      if (!alias) return;
      try {
        await saveBlockExample(filePath, alias, name, JSON.stringify(response));
        setExamplesRefreshTick((t) => t + 1);
      } catch {
        /* Best-effort. */
      }
    },
    [alias, filePath],
  );

  const restoreExample = useCallback(
    (ex: BlockExample) => {
      try {
        const restored = JSON.parse(ex.response_json) as HttpResponseFull;
        // No elapsed to show for a restored example → omit
        // durationMs (leave it untouched, as the old inline setters
        // did). applyCachedResult covers response + success + cached
        // + error-clear.
        applyCachedResult(restored);
        setLastRunAt(new Date(ex.saved_at));
        closeDrawer();
      } catch {
        /* Bad JSON in stored example — ignore. */
      }
    },
    [applyCachedResult, setLastRunAt, closeDrawer],
  );

  const deleteExample = useCallback(async (id: number) => {
    try {
      await deleteBlockExample(id);
      setExamplesRefreshTick((t) => t + 1);
    } catch {
      /* Best-effort. */
    }
  }, []);

  return {
    historyEntries,
    examples,
    recordHistory,
    bumpHistoryTick,
    purgeHistory,
    saveExample,
    restoreExample,
    deleteExample,
  };
}
