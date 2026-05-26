import { useCallback, useRef, useState } from "react";

import { collectBlocksAboveCM } from "@/lib/blocks/document";
import type { BlockContext } from "@/lib/blocks/references";
import { useEnvironmentStore } from "@/stores/environment";
import type { EditorView } from "@codemirror/view";

import type { ExecutionState } from "@/components/blocks/http/fenced/shared";

/** Result of one streamed execution, normalized across block types. */
export type ExecOutcome<T> =
  | { status: "success"; response: T }
  | { status: "error"; message: string }
  | { status: "cancelled" };

/** Context handed to `prepare` — the blocks above this one (for
 *  `{{alias.response.path}}` resolution) plus the active environment's
 *  variables (for `{{ENV_KEY}}`). The hook collects both once per run. */
export interface PrepareCtx {
  blocksAbove: BlockContext[];
  envVars: Record<string, string>;
  blockFrom: number;
}

export interface UseExecutableBlockArgs<T, P = Record<string, unknown>> {
  /** `"http"` | `"db"` — execution-id prefix only. */
  idPrefix: string;
  blockId: string;
  view: EditorView;
  blockFrom: number;
  filePath: string;

  /** Synchronous pre-flight gate, run *before* the FSM enters
   *  `running` (so an invalid block never flashes running / spawns an
   *  AbortController / hits the doc). Return an error message to abort,
   *  or `null` to proceed. */
  validate?: () => string | null;

  /** Resolve `{{refs}}` and build the executor params. Runs after the
   *  FSM is `running` and the blocks/env are collected. Return
   *  `{ error }` to fail (FSM → error, duration stamped) or
   *  `{ params }` to dispatch. `P` is the block-specific param shape
   *  (HTTP `ExecutorParams`, DB query params) — kept generic so
   *  neither contract is widened to `Record<string, unknown>`. */
  prepare: (ctx: PrepareCtx) => Promise<{ params: P } | { error: string }>;

  /** Dispatch the streamed backend call. */
  execute: (a: {
    executionId: string;
    params: P;
    signal: AbortSignal;
    onProgress?: (bytes: number) => void;
  }) => Promise<ExecOutcome<T>>;

  /** Pull the server-measured elapsed out of a success response (HTTP
   *  `elapsed_ms`, DB `stats.elapsed_ms`); falls back to the wall
   *  clock when undefined. */
  elapsedOf: (response: T) => number | undefined;

  /** Optional cache write on success. `ctx` carries the env snapshot
   *  so the cache key can be env-scoped. Best-effort by contract — the
   *  hook never lets a persist failure surface. */
  persist?: (
    response: T,
    elapsed: number,
    ctx: { envVars: Record<string, string> },
  ) => Promise<void>;

  /** Per-outcome side effect (run-history rows, last-run stamp). Fired
   *  for success/error/cancelled after the FSM transition. */
  onOutcome?: (outcome: ExecOutcome<T>, elapsed: number) => void;

  /** Called once the FSM enters `running` (e.g. reset a download
   *  counter). */
  onRunStart?: () => void;

  /** Streamed progress (cumulative bytes) — forwarded to `execute`. */
  onProgress?: (bytes: number) => void;
}

export interface UseExecutableBlockResult<T> {
  executionState: ExecutionState;
  response: T | null;
  error: string | null;
  durationMs: number | null;
  cached: boolean;
  run: () => Promise<void>;
  cancel: () => void;
  /** Push a stored success into the FSM without executing — used by
   *  the per-panel cache-hydrate effect (divergent normalization stays
   *  in the panel) and the restore-from-example flow. Sets
   *  response/state=success/cached + clears error; `durationMs`
   *  omitted leaves the duration untouched, a value (incl. null) sets
   *  it. */
  applyCachedResult: (response: T, durationMs?: number | null) => void;
  /** Back to idle (clears response/error/duration/cached). */
  reset: () => void;
}

/**
 * The execution state machine shared by the HTTP and DB block panels
 * (audit 03 §1 #4 — `runBlock` ≈ `executeRun`, ~150 L duplicated).
 *
 * The hook owns ONLY the genuinely-identical skeleton: the
 * `idle→running→success|error|cancelled` FSM + `response/error/
 * durationMs/cached`, the AbortController lifecycle, the
 * collect-blocks-above + active-env gather, the elapsed bookkeeping,
 * and the try/catch/finally shell. Everything that genuinely diverges
 * between HTTP and DB — validation, ref resolution + param assembly,
 * the streamed executor, the cache hash/write, run-history, the
 * progress indicator — is injected via the adapter (RULE 4: do not
 * distort either contract to force a merge; the divergent
 * cache-*hydrate* stays per-panel by design, only the *write* is
 * pluggable here).
 */
export function useExecutableBlock<T, P = Record<string, unknown>>(
  args: UseExecutableBlockArgs<T, P>,
): UseExecutableBlockResult<T> {
  const {
    idPrefix,
    blockId,
    view,
    blockFrom,
    filePath,
    validate,
    prepare,
    execute,
    elapsedOf,
    persist,
    onOutcome,
    onRunStart,
    onProgress,
  } = args;

  const [executionState, setExecutionState] = useState<ExecutionState>("idle");
  const [response, setResponse] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [durationMs, setDurationMs] = useState<number | null>(null);
  const [cached, setCached] = useState(false);

  const abortRef = useRef<AbortController | null>(null);

  const cancel = useCallback(() => {
    abortRef.current?.abort();
    abortRef.current = null;
  }, []);

  const applyCachedResult = useCallback((r: T, d?: number | null) => {
    setResponse(r);
    setExecutionState("success");
    setCached(true);
    setError(null);
    // `undefined` → leave duration untouched (the restore-example
    // flow has no elapsed to show); a value (incl. null) sets it
    // (the cache-hydrate flow).
    if (d !== undefined) setDurationMs(d);
  }, []);

  const reset = useCallback(() => {
    setExecutionState("idle");
    setResponse(null);
    setError(null);
    setDurationMs(null);
    setCached(false);
  }, []);

  // The terminal FSM transition for one outcome. Extracted from `run`
  // so `run` stays under the cyclomatic-complexity bar — the
  // success/error/cancelled fan-out is the bulk of the branches.
  // Returns whether the run succeeded (the caller gates `persist` on
  // it — only successes are cached, matching the prior inline flow).
  const commitOutcome = useCallback(
    (outcome: ExecOutcome<T>, elapsed: number): boolean => {
      if (outcome.status === "cancelled") {
        setExecutionState("cancelled");
        setDurationMs(elapsed);
      } else if (outcome.status === "error") {
        setError(outcome.message);
        setExecutionState("error");
        setDurationMs(elapsed);
      } else {
        setResponse(outcome.response);
        setDurationMs(elapsedOf(outcome.response) || elapsed);
        setExecutionState("success");
      }
      onOutcome?.(outcome, elapsed);
      return outcome.status === "success";
    },
    [elapsedOf, onOutcome],
  );

  const run = useCallback(async () => {
    if (executionState === "running") return;

    const gate = validate?.() ?? null;
    if (gate !== null) {
      setError(gate);
      setExecutionState("error");
      return;
    }

    setError(null);
    setCached(false);
    setExecutionState("running");
    onRunStart?.();

    const abort = new AbortController();
    abortRef.current = abort;
    const executionId = `${idPrefix}_${blockId}_${Date.now()}`;
    const startedAt = performance.now();

    try {
      const blocksAbove = await collectBlocksAboveCM(
        view.state.doc,
        blockFrom,
        filePath,
      );
      const envVars = await useEnvironmentStore.getState().getActiveVariables();

      const prepared = await prepare({ blocksAbove, envVars, blockFrom });
      if ("error" in prepared) {
        setError(prepared.error);
        setExecutionState("error");
        setDurationMs(Math.round(performance.now() - startedAt));
        return;
      }

      const outcome = await execute({
        executionId,
        params: prepared.params,
        signal: abort.signal,
        onProgress,
      });
      const elapsed = Math.round(performance.now() - startedAt);
      const ok = commitOutcome(outcome, elapsed);

      if (ok && outcome.status === "success" && persist) {
        try {
          await persist(
            outcome.response,
            elapsedOf(outcome.response) || elapsed,
            { envVars },
          );
        } catch {
          // Cache write is best-effort.
        }
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setExecutionState("error");
    } finally {
      abortRef.current = null;
    }
  }, [
    executionState,
    validate,
    onRunStart,
    idPrefix,
    blockId,
    view,
    blockFrom,
    filePath,
    prepare,
    execute,
    elapsedOf,
    persist,
    commitOutcome,
    onProgress,
  ]);

  return {
    executionState,
    response,
    error,
    durationMs,
    cached,
    run,
    cancel,
    applyCachedResult,
    reset,
  };
}
