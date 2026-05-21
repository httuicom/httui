/**
 * Cached `{{ref}}` autocomplete context for the form-mode inline
 * editors of an HTTP block. Extracted from `HttpFencedPanel.tsx`
 * during the follow-up to A1+A2a so the orchestrator stays < 600 L.
 *
 * Refreshes the snapshot whenever the doc-structure key changes
 * (`block.from` shifts when blocks are added/removed above; the
 * source doc swaps when CM6 rebuilds the StateField). Storage is a
 * ref + a memo of stable accessors so the autocomplete extension
 * reads always-fresh data without forcing the panel to re-render on
 * every CM6 transaction.
 */

import { useEffect, useMemo, useRef } from "react";
import type { EditorView } from "@codemirror/view";

import { collectBlocksAboveCM } from "@/lib/blocks/document";
import { useEnvironmentStore } from "@/stores/environment";
import type { BlockContext } from "@/lib/blocks/references";
import type { EnvKeyInfo } from "@/lib/blocks/cm-autocomplete";

export interface HttpRefsGetters {
  getBlocks: () => BlockContext[];
  getEnvKeys: () => (string | EnvKeyInfo)[];
}

export function useHttpRefsContext(
  view: EditorView,
  blockFrom: number,
  filePath: string,
): HttpRefsGetters {
  const refsCtxRef = useRef<{
    blocks: BlockContext[];
    envKeys: (string | EnvKeyInfo)[];
  }>({ blocks: [], envKeys: [] });

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const blocks = await collectBlocksAboveCM(
          view.state.doc,
          blockFrom,
          filePath,
        );
        const env = await useEnvironmentStore.getState().getActiveVariables();
        if (cancelled) return;
        refsCtxRef.current = {
          blocks,
          envKeys: Object.keys(env),
        };
      } catch {
        /* best-effort */
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [blockFrom, filePath, view.state.doc]);

  return useMemo(
    () => ({
      getBlocks: () => refsCtxRef.current.blocks,
      getEnvKeys: () => refsCtxRef.current.envKeys,
    }),
    [],
  );
}
