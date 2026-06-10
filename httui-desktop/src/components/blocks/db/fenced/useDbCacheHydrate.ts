// On-mount cache hydration: when a cached row exists for the current
// (body, connection, env snapshot) hash, surface it as the panel's
// result without running the query.
import { useEffect } from "react";

import { computeDbCacheHash } from "@/lib/blocks/hash";
import { getBlockResult } from "@/lib/tauri/commands";
import { useEnvironmentStore } from "@/stores/environment";
import type { DbResponse } from "@/components/blocks/db/types";
import { type ExecutionState } from "./shared";

export function useDbCacheHydrate(opts: {
  filePath: string;
  body: string;
  connId: string;
  onHit: (hit: {
    response: DbResponse;
    elapsedMs: number | null;
    state: ExecutionState;
  }) => void;
}) {
  const { filePath, body, connId, onHit } = opts;
  useEffect(() => {
    if (!filePath) return;
    if (!connId || !body.trim()) return;

    let cancelled = false;
    (async () => {
      try {
        const envVars = await useEnvironmentStore
          .getState()
          .getActiveVariables();
        const hash = await computeDbCacheHash(body, connId, envVars);
        const row = await getBlockResult(filePath, hash);
        if (cancelled || !row) return;
        const parsed = JSON.parse(row.response) as DbResponse;
        onHit({
          response: parsed,
          elapsedMs: row.elapsed_ms ?? null,
          state: row.status === "success" ? "success" : "error",
        });
      } catch {
        // Cache miss or corrupt — stay idle.
      }
    })();
    return () => {
      cancelled = true;
    };
    // onHit is a stable setter bundle owned by the panel — deliberately
    // not a dependency so a re-created callback can't re-trigger reads.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filePath, body, connId]);
}
