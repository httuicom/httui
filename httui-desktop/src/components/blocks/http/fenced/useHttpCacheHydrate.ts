/**
 * Hydrate the HTTP block's FSM from the SQLite cache on mount + on
 * each body change. Extracted from `HttpFencedPanel.tsx` during the
 * follow-up to A1+A2a so the orchestrator stays < 600 L.
 *
 * Mutations (POST/PUT/PATCH/DELETE) are skipped — re-running a
 * destructive request without a fresh user click is unsafe. The
 * hash/normalize is HTTP-specific (lives here); only the FSM push
 * goes through `useExecutableBlock`'s `applyCachedResult`.
 */

import { useEffect } from "react";

import { getBlockResult } from "@/lib/tauri/commands";
import {
  normalizeHttpResponse,
  type HttpResponseFull,
} from "@/lib/tauri/streamedExecution";
import { computeHttpCacheHash } from "@/lib/blocks/hash";
import { useEnvironmentStore } from "@/stores/environment";
import type { HttpMessageParsed } from "@/lib/blocks/http-message";
import { MUTATION_METHODS } from "./shared";

interface UseHttpCacheHydrateParams {
  parsed: HttpMessageParsed;
  filePath: string;
  applyCachedResult: (response: HttpResponseFull, elapsed?: number) => void;
  setLastRunAt: (d: Date | null) => void;
}

export function useHttpCacheHydrate({
  parsed,
  filePath,
  applyCachedResult,
  setLastRunAt,
}: UseHttpCacheHydrateParams): void {
  useEffect(() => {
    if (MUTATION_METHODS.has(parsed.method)) return;
    if (!parsed.url || !parsed.url.trim()) return;
    let cancelled = false;
    void (async () => {
      try {
        const envVars = await useEnvironmentStore
          .getState()
          .getActiveVariables();
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
        const hit = await getBlockResult(filePath, hash);
        if (cancelled || !hit) return;
        try {
          const stored = JSON.parse(hit.response) as unknown;
          const norm = normalizeHttpResponse(stored);
          applyCachedResult(norm, norm.elapsed_ms || hit.elapsed_ms);
          setLastRunAt(hit.executed_at ? new Date(hit.executed_at) : null);
        } catch {
          // Ignore corrupt cache entries.
        }
      } catch {
        // Cache lookup is best-effort.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [parsed, filePath, applyCachedResult, setLastRunAt]);
}
