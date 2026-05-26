/**
 * Pre-compute code-generation snippets (cURL / fetch / Python / HTTPie
 * / .http file) for an HTTP block, refreshed whenever the parsed body
 * or environment context changes. Extracted from `HttpFencedPanel.tsx`
 * during the follow-up to A1+A2a so the orchestrator stays < 600 L.
 *
 * We HAVE to pre-compute because the browser's clipboard API requires
 * a *user gesture* — `await`-ing on `collectBlocksAboveCM` /
 * `getActiveVariables` inside the click handler loses that gesture
 * context and the call silently fails. Holding the resolved snippets
 * in state lets the click handler call `writeText` synchronously
 * inside the gesture window.
 *
 * `handleSendAs("http-file")` opens a save dialog instead (async path
 * with no clipboard gesture to preserve); the other formats route to
 * `navigator.clipboard.writeText`. `copyAsCurl` is the keyboard-
 * shortcut (`Mod-Shift-c`) shortcut to the cURL path.
 */

import { useCallback, useEffect, useState } from "react";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import { writeFile } from "@tauri-apps/plugin-fs";
import type { EditorView } from "@codemirror/view";

import { collectBlocksAboveCM } from "@/lib/blocks/document";
import { resolveAllReferences } from "@/lib/blocks/references";
import { useEnvironmentStore } from "@/stores/environment";
import {
  toCurl,
  toFetch,
  toHTTPie,
  toHttpFile,
  toPython,
} from "@/lib/blocks/http-codegen";
import type { HttpMessageParsed } from "@/lib/blocks/http-fence";
import type { SendAsFormat } from "./shared";

interface UseHttpCodegenSnippetsParams {
  view: EditorView;
  blockFrom: number;
  filePath: string;
  parsed: HttpMessageParsed;
  alias: string | undefined;
}

interface UseHttpCodegenSnippetsResult {
  snippets: Record<SendAsFormat, string> | null;
  handleSendAs: (format: SendAsFormat) => void;
  copyAsCurl: () => void;
}

export function useHttpCodegenSnippets({
  view,
  blockFrom,
  filePath,
  parsed,
  alias,
}: UseHttpCodegenSnippetsParams): UseHttpCodegenSnippetsResult {
  const [snippets, setSnippets] = useState<Record<SendAsFormat, string> | null>(
    null,
  );

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const blocksAbove = await collectBlocksAboveCM(
          view.state.doc,
          blockFrom,
          filePath,
        );
        const envVars = await useEnvironmentStore
          .getState()
          .getActiveVariables();
        if (cancelled) return;
        const resolveText = (text: string) =>
          resolveAllReferences(text, blocksAbove, blockFrom, envVars).resolved;
        const resolved = {
          method: parsed.method,
          url: resolveText(parsed.url),
          params: parsed.params.map((p) => ({
            ...p,
            key: resolveText(p.key),
            value: resolveText(p.value),
          })),
          headers: parsed.headers.map((h) => ({
            ...h,
            key: resolveText(h.key),
            value: resolveText(h.value),
          })),
          body: parsed.body ? resolveText(parsed.body) : "",
        };
        if (cancelled) return;
        setSnippets({
          curl: toCurl(resolved),
          fetch: toFetch(resolved),
          python: toPython(resolved),
          httpie: toHTTPie(resolved),
          "http-file": toHttpFile(resolved),
        });
      } catch {
        if (!cancelled) setSnippets(null);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [blockFrom, filePath, parsed, view.state.doc]);

  const handleSendAs = useCallback(
    (format: SendAsFormat) => {
      const snippet = snippets?.[format];
      if (!snippet) return;

      if (format === "http-file") {
        // Save dialog flow runs async — no clipboard gesture to preserve.
        void (async () => {
          try {
            const defaultName = `${alias ?? "request"}.http`;
            const path = await saveDialog({
              defaultPath: defaultName,
              filters: [{ name: "HTTP request", extensions: ["http", "rest"] }],
            });
            if (!path) return;
            await writeFile(path, new TextEncoder().encode(snippet));
          } catch (e) {
            window.alert(
              `Failed to save: ${e instanceof Error ? e.message : String(e)}`,
            );
          }
        })();
        return;
      }

      // Synchronous call from inside the click handler — gesture context
      // is still active here.
      navigator.clipboard.writeText(snippet).catch(() => {
        /* clipboard denied — user can retry */
      });
    },
    [alias, snippets],
  );

  const copyAsCurl = useCallback(() => {
    handleSendAs("curl");
  }, [handleSendAs]);

  return { snippets, handleSendAs, copyAsCurl };
}
