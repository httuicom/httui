/**
 * React panel for an `http` fenced block (stage 4 of the redesign).
 *
 * Lives outside CM6's document flow: the CM extension `cm-http-block.tsx`
 * registers three container divs per block (toolbar, result, statusbar),
 * and this component mounts React into each via `createPortal`. The
 * settings drawer uses a Chakra Portal anchored to document.body (not
 * Dialog — would trap focus away from CM6).
 *
 * Execution runs through `executeHttpStreamed` (stage 2 plumbing).
 * Results are persisted to the SQLite block-result cache hashed by
 * method + URL + headers + body + env-snapshot. Mutation methods
 * (POST/PUT/PATCH/DELETE) are NEVER served from cache — they always
 * re-execute.
 *
 * The orchestrator delegates four cohesive concerns to sibling hooks:
 * `useHttpRefsContext` (autocomplete refs), `useHttpCacheHydrate`
 * (on-mount cache lookup), `useHttpCodegenSnippets` (cURL/fetch/…
 * pre-comp + Send-As), `useHttpDrawerData` (history/examples loading
 * + recordHistory + drawer actions). The pure module helpers live in
 * `./http-request-builder.ts`.
 */

import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";

import {
  setHttpBlockActions,
  type HttpPortalEntry,
} from "@/lib/codemirror/cm-http-block";
import {
  stringifyHttpFenceInfo,
  stringifyHttpMessageBody,
  type HttpBlockMetadata,
  type HttpMessageParsed,
} from "@/lib/blocks/http-message";
import {
  deriveBodyMode,
  isCompatibleSwitch,
  setContentTypeForMode,
  type HttpBodyMode,
} from "@/lib/blocks/http-body-modes";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { useBlockSettings } from "./useBlockSettings";
import {
  buildExecutorParams,
  deriveHost,
  httpElapsedOf,
  parseBody,
} from "./http-request-builder";
import { useHttpRefsContext } from "./useHttpRefsContext";
import { useHttpCacheHydrate } from "./useHttpCacheHydrate";
import { useHttpCodegenSnippets } from "./useHttpCodegenSnippets";
import { useHttpDrawerData } from "./useHttpDrawerData";
import { useExecutableBlock } from "@/hooks/useExecutableBlock";
import { HttpBodyView } from "./HttpBodyView";
import { HttpInlineCM } from "./HttpInlineEditors";
import { HttpBodyByMode } from "./HttpFormTables";
import { toaster } from "@/components/ui/toaster";
import {
  cancelBlockExecution,
  executeHttpStreamed,
  type HttpResponseFull,
} from "@/lib/tauri/streamedExecution";
import { resolveAllReferences } from "@/lib/blocks/references";
import { computeHttpCacheHash } from "@/lib/blocks/hash";
import { EditorView } from "@codemirror/view";

import { saveBlockResult } from "@/lib/tauri/commands";
import { notifyBlockRan } from "@/lib/lsp/client";
import type { BlockContext } from "@/lib/blocks/references";

interface HttpFencedPanelProps {
  blockId: string;
  block: HttpPortalEntry["block"];
  entry: HttpPortalEntry;
  view: EditorView;
  filePath: string;
}

// ExecutionState / METHOD_COLORS / MUTATION_METHODS / SendAsFormat moved to ./shared.ts
import { MUTATION_METHODS } from "./shared";
import { HttpToolbar } from "./HttpToolbar";

// statusDotColor / formatBytes / relativeTimeAgo live in ./shared.ts
// (formatBytes now consumed by HttpBodyView, not the panel).
import { HttpStatusBar } from "./HttpStatusBar";
import { HttpResultTabs } from "./HttpResultTabs";
import { HttpFormMode } from "./HttpFormMode";
import { HttpSettingsDrawer } from "./HttpSettingsDrawer";

// parseBody / deriveHost / httpElapsedOf / isValidHeaderName /
// buildExecutorParams moved to ./http-request-builder.ts (pure module
// helpers, testable in isolation — see __tests__).

// ─────────────────────── Sub-components ───────────────────────
// HttpToolbar moved to ./HttpToolbar.tsx

// bodyAsText moved to ./shared.ts

// HttpStatusBar moved to ./HttpStatusBar.tsx

// ─────────────────────── Main panel ───────────────────────

export const HttpFencedPanel = memo(function HttpFencedPanel({
  blockId,
  block,
  entry,
  view,
  filePath,
}: HttpFencedPanelProps) {
  const parsed = useMemo(() => parseBody(block.body), [block.body]);
  const host = useMemo(() => deriveHost(parsed.url), [parsed.url]);

  // executionState / response / error / durationMs / cached + the
  // AbortController + run/cancel are owned by useExecutableBlock (A2);
  // wired below once recordHistory + the adapter callbacks exist.
  const [lastRunAt, setLastRunAt] = useState<Date | null>(null);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [settings, setSettings] = useBlockSettings(
    filePath,
    block.metadata.alias,
  );
  // Cumulative bytes received during a streamed response. Only meaningful
  // while `executionState === "running"` and stays at 0 for fast responses
  // that finish before any BodyChunk fires. Reset on every new run.
  const [downloadingBytes, setDownloadingBytes] = useState(0);

  // Cached `{{ref}}` autocomplete context for the form-mode inputs —
  // refresh-on-doc-structure-change + stable getters that the
  // autocomplete extension reads without forcing a re-render.
  const refsGetters = useHttpRefsContext(view, block.from, filePath);

  // ── Drawer data (uses a ref for `applyCachedResult` since the FSM
  // hook hasn't run yet; populated by the `useEffect` after
  // `useExecutableBlock`). The ref is read at click-time, so by the
  // user actually invoking `restoreExample` the FSM is mounted. ──
  const applyCachedResultRef = useRef<
    ((r: HttpResponseFull, elapsed?: number) => void) | null
  >(null);
  const applyCachedResultStable = useCallback(
    (r: HttpResponseFull, elapsed?: number) =>
      applyCachedResultRef.current?.(r, elapsed),
    [],
  );
  const closeDrawer = useCallback(() => setDrawerOpen(false), []);
  const {
    historyEntries,
    examples,
    recordHistory,
    bumpHistoryTick,
    purgeHistory,
    saveExample,
    restoreExample,
    deleteExample,
  } = useHttpDrawerData({
    filePath,
    alias: block.metadata.alias,
    drawerOpen,
    settings,
    applyCachedResult: applyCachedResultStable,
    setLastRunAt,
    closeDrawer,
  });

  // ── Execution (A2: shared FSM via useExecutableBlock) ──
  // The idle→running→success|error|cancelled machine + AbortController +
  // collect-blocks/env + try/catch/finally live in the hook. The HTTP-
  // specific pieces are the adapter below.
  const validate = useCallback(
    () => (!parsed.url || !parsed.url.trim() ? "URL is required" : null),
    [parsed.url],
  );

  const prepare = useCallback(
    async ({
      blocksAbove,
      envVars,
      blockFrom,
    }: {
      blocksAbove: BlockContext[];
      envVars: Record<string, string>;
      blockFrom: number;
    }) => {
      const resolveText = (text: string) =>
        resolveAllReferences(text, blocksAbove, blockFrom, envVars).resolved;
      const { params, errors } = buildExecutorParams(
        parsed,
        resolveText,
        block.metadata.timeoutMs,
        settings,
      );
      if (errors.length > 0) return { error: errors.join("\n") };
      return { params };
    },
    [parsed, block.metadata.timeoutMs, settings],
  );

  // Mutations (POST/PUT/PATCH/DELETE) are never cached — re-executed
  // every time so we never serve a stale destructive result.
  const persist = useCallback(
    async (
      resp: HttpResponseFull,
      elapsed: number,
      { envVars }: { envVars: Record<string, string> },
    ) => {
      if (MUTATION_METHODS.has(parsed.method)) return;
      const hash = await computeHttpCacheHash(
        {
          method: parsed.method,
          url: parsed.url,
          params: parsed.params
            .filter((p) => p.enabled)
            .map((p) => ({ key: p.key, value: p.value })),
          headers: parsed.headers
            .filter((h) => h.enabled)
            .map((h) => ({ key: h.key, value: h.value })),
          body: parsed.body,
        },
        envVars,
      );
      await saveBlockResult(
        filePath,
        hash,
        "success",
        JSON.stringify(resp),
        elapsed,
        null,
        block.metadata.alias ?? null,
      );
      // An aliased success refreshes the inferred shape — let the
      // language server republish field diagnostics against it.
      if (block.metadata.alias) notifyBlockRan();
    },
    [parsed, filePath, block.metadata.alias],
  );

  const onOutcome = useCallback(
    (
      outcome:
        | { status: "success"; response: HttpResponseFull }
        | { status: "error"; message: string }
        | { status: "cancelled" },
      elapsed: number,
    ) => {
      if (outcome.status === "success") {
        setLastRunAt(new Date());
        void recordHistory({
          method: parsed.method,
          url: parsed.url,
          status: outcome.response.status_code,
          requestSize: parsed.body.length || null,
          responseSize: outcome.response.size_bytes,
          elapsedMs: outcome.response.elapsed_ms || elapsed,
          outcome: "success",
        });
        return;
      }
      void recordHistory({
        method: parsed.method,
        url: parsed.url,
        status: null,
        requestSize: parsed.body.length || null,
        responseSize: null,
        elapsedMs: elapsed,
        outcome: outcome.status === "cancelled" ? "cancelled" : "error",
      });
    },
    [parsed, recordHistory],
  );

  const onRunStart = useCallback(() => setDownloadingBytes(0), []);
  const onProgress = useCallback(
    (bytes: number) => setDownloadingBytes(bytes),
    [],
  );

  const {
    executionState,
    response,
    error,
    durationMs,
    cached,
    run: runBlock,
    cancel: cancelBlock,
    applyCachedResult,
  } = useExecutableBlock<HttpResponseFull>({
    idPrefix: "http",
    blockId,
    view,
    blockFrom: block.from,
    filePath,
    validate,
    prepare,
    execute: executeHttpStreamed,
    elapsedOf: httpElapsedOf,
    persist,
    onOutcome,
    onRunStart,
    onProgress,
  });

  // Wire the ref so `drawer.restoreExample` can call `applyCachedResult`.
  useEffect(() => {
    applyCachedResultRef.current = applyCachedResult;
  }, [applyCachedResult]);

  // Hydrate from cache on mount / body change.
  useHttpCacheHydrate({ parsed, filePath, applyCachedResult, setLastRunAt });

  const onOpenSettings = useCallback(() => {
    setDrawerOpen(true);
    bumpHistoryTick();
  }, [bumpHistoryTick]);

  // Pre-computed cURL / fetch / Python / HTTPie / .http snippets +
  // the Send-As + ⌘⇧c handlers — see `useHttpCodegenSnippets`.
  const { handleSendAs, copyAsCurl } = useHttpCodegenSnippets({
    view,
    blockFrom: block.from,
    filePath,
    parsed,
    alias: block.metadata.alias,
  });

  useEffect(() => {
    setHttpBlockActions(blockId, {
      onRun: () => void runBlock(),
      onCancel: cancelBlock,
      onOpenSettings,
      onCopyAsCurl: copyAsCurl,
    });
  }, [blockId, runBlock, cancelBlock, onOpenSettings, copyAsCurl]);

  // Cancel any in-flight run if the panel unmounts or the block id
  // changes after a doc-level edit. `cancelBlock` (from the hook)
  // aborts the in-flight controller; we also tell the backend.
  useEffect(() => {
    return () => {
      cancelBlock();
      void cancelBlockExecution(`http_${blockId}`);
    };
  }, [blockId, cancelBlock]);

  // ── Drawer actions ──
  const updateMetadata = useCallback(
    (patch: Partial<HttpBlockMetadata>) => {
      const next: HttpBlockMetadata = { ...block.metadata, ...patch };
      const infoText = stringifyHttpFenceInfo(next);
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
    const from = block.from;
    const to = Math.min(block.to + 1, view.state.doc.length);
    view.dispatch({ changes: { from, to, insert: "" } });
    setDrawerOpen(false);
  }, [block.from, block.to, view]);

  // "Save body as variable" was removed — block references
  // (`{{alias.response.path}}`) cover the same use case more cleanly. A
  // proper context-menu-on-JSON-value with path-aware "Save as variable"
  // is reserved for a follow-up (see http-block-redesign §2.5).

  // ── Form-mode editing: re-emit raw body whenever the form changes ──
  const replaceBody = useCallback(
    (nextRaw: string) => {
      // bodyFrom is the start of the first body line; bodyTo is end of the
      // last body line. Replace the whole range with the new canonical raw.
      view.dispatch({
        changes: {
          from: block.bodyFrom,
          to: block.bodyTo,
          insert: nextRaw,
        },
      });
    },
    [block.bodyFrom, block.bodyTo, view],
  );

  const onFormChange = useCallback(
    (next: HttpMessageParsed) => {
      replaceBody(stringifyHttpMessageBody(next));
    },
    [replaceBody],
  );

  const onToggleMode = useCallback(
    (next: "raw" | "form") => {
      if (next === (block.metadata.mode ?? "raw")) return;
      // Persist mode in the info string. Default raw is omitted; form is
      // explicit. Re-stringify the body too so toggling raw → form → raw
      // is a fixed point (canonical reformat).
      const reformatted = stringifyHttpMessageBody(parsed);
      // If the mode changed, also reformat the body to keep the contract
      // "form re-parses raw on each flip" idempotent.
      if (reformatted !== block.body) {
        replaceBody(reformatted);
      }
      const meta: HttpBlockMetadata = {
        ...block.metadata,
        mode: next === "raw" ? undefined : "form",
      };
      const infoText = stringifyHttpFenceInfo(meta);
      const openLine = view.state.doc.lineAt(block.openLineFrom);
      view.dispatch({
        changes: {
          from: openLine.from,
          to: openLine.to,
          insert: "```" + infoText,
        },
      });
    },
    [block.body, block.metadata, block.openLineFrom, parsed, replaceBody, view],
  );

  const onPickBodyMode = useCallback(
    (next: HttpBodyMode) => {
      const prev = deriveBodyMode(parsed.headers);
      if (prev === next) return;
      if (!isCompatibleSwitch(prev, next, parsed.body)) {
        toaster.create({
          type: "warning",
          title: "Body may be incompatible",
          description: `Switched to ${next}. Existing body still looks like ${prev}; clear or replace it if it doesn't match.`,
        });
      }
      const updated = setContentTypeForMode(parsed, next);
      replaceBody(stringifyHttpMessageBody(updated));
    },
    [parsed, replaceBody],
  );

  /** Open the OS-native file picker; returns the absolute path or `null` if
   *  the user cancelled. Lives at the panel root so the form-mode body tab
   *  doesn't have to import Tauri APIs directly. */
  const pickFile = useCallback(async (): Promise<string | null> => {
    try {
      const result = await openFileDialog({ multiple: false });
      return typeof result === "string" ? result : null;
    } catch {
      return null;
    }
  }, []);

  const toolbarNode = entry.toolbar;
  const formNode = entry.form;
  const resultNode = entry.result;
  const statusbarNode = entry.statusbar;
  const currentMode: "raw" | "form" =
    block.metadata.mode === "form" ? "form" : "raw";
  const currentBodyMode = deriveBodyMode(parsed.headers);

  return (
    <>
      {toolbarNode &&
        createPortal(
          <HttpToolbar
            alias={block.metadata.alias}
            method={parsed.method}
            host={host}
            mode={currentMode}
            bodyMode={currentBodyMode}
            executionState={executionState}
            onRun={() => void runBlock()}
            onCancel={cancelBlock}
            onOpenSettings={onOpenSettings}
            onToggleMode={onToggleMode}
            onPickBodyMode={onPickBodyMode}
          />,
          toolbarNode,
        )}

      {formNode &&
        createPortal(
          <HttpFormMode
            parsed={parsed}
            bodyMode={currentBodyMode}
            onChange={onFormChange}
            onPickFile={pickFile}
            refsGetters={refsGetters}
            InlineCM={HttpInlineCM}
            renderBodyTab={({
              parsed: p,
              onCommit,
              onPickFile: pick,
              refsGetters: r,
            }) => (
              <HttpBodyByMode
                bodyMode={currentBodyMode}
                parsed={p}
                onCommit={onCommit}
                onPickFile={pick}
                refsGetters={r}
              />
            )}
          />,
          formNode,
        )}

      {resultNode &&
        createPortal(
          <HttpResultTabs
            executionState={executionState}
            response={response}
            error={error}
            cached={cached}
            bodyView={(rawBody, prettyBody, resp) => (
              <HttpBodyView
                rawBody={rawBody}
                prettyBody={prettyBody}
                response={resp}
              />
            )}
          />,
          resultNode,
        )}

      {statusbarNode &&
        createPortal(
          <HttpStatusBar
            alias={block.metadata.alias}
            host={host}
            executionState={executionState}
            response={response}
            durationMs={durationMs}
            cached={cached}
            lastRunAt={lastRunAt}
            downloadingBytes={downloadingBytes}
            onSendAs={handleSendAs}
          />,
          statusbarNode,
        )}

      {drawerOpen && (
        <HttpSettingsDrawer
          metadata={block.metadata}
          history={historyEntries}
          examples={examples}
          settings={settings}
          canSaveExample={!!response && !!block.metadata.alias}
          onClose={closeDrawer}
          onUpdateMetadata={updateMetadata}
          onUpdateSettings={setSettings}
          onDelete={deleteBlockFromDoc}
          onPurgeHistory={purgeHistory}
          onSaveExample={async (name) => {
            if (!response) return;
            await saveExample(name, response);
          }}
          onRestoreExample={restoreExample}
          onDeleteExample={deleteExample}
        />
      )}
    </>
  );
});
