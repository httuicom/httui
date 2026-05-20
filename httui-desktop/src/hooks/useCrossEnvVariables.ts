import { useEffect, useState } from "react";

import { useEnvironmentStore } from "@/stores/environment";
import {
  listEnvVariables,
  type Environment,
  type EnvVariable,
} from "@/lib/tauri/commands";

export interface EnvVarsBundle {
  env: Environment;
  vars: EnvVariable[];
}

/**
 * Fan-out load of every environment's variables in parallel.
 *
 * This is the genuinely-shared async unit between
 * `EnvironmentsPageContainer` (derives `summaries` + `secretCounts`)
 * and `VariablesPageContainer` (derives cross-env `rows` via
 * `mergeCrossEnvVariables`). Both used to re-implement this exact
 * `Promise.all(environments.map(listEnvVariables))` effect, with the
 * cancelled-guard boilerplate, then apply a page-specific derivation.
 *
 * The derivation stays page-specific *on purpose*: it is recomputed
 * synchronously via `useMemo` over the returned bundles. A Zustand
 * selector can't do this — the fan-out is async (audit 05 §A.3). So
 * the async load is centralized here; the per-page shaping stays a
 * pure memo at the call-site.
 *
 * Per-env failure is swallowed (`catch → []`) so one bad env doesn't
 * blank the whole page — matches the prior behavior verbatim.
 *
 * NOT consumed by `EnvironmentManager`: that drawer loads a *single*
 * selected env via the store's `loadVariables` action (a different
 * contract — single-env, store-action with its own side effects), not
 * this cross-env IPC fan-out. Folding it in would change behavior, so
 * it is deliberately left as-is.
 *
 * Mount refresh + the `config-changed` listener stay at the call-site
 * (`useConfigSyncedResource`) — out of scope for this hook.
 */
export function useCrossEnvVariables(): EnvVarsBundle[] {
  const environments = useEnvironmentStore((s) => s.environments);
  const variablesVersion = useEnvironmentStore((s) => s.variablesVersion);
  const [bundles, setBundles] = useState<EnvVarsBundle[]>([]);

  useEffect(() => {
    let cancelled = false;
    if (environments.length === 0) {
      setBundles([]);
      return;
    }
    void Promise.all(
      environments.map(async (env) => ({
        env,
        vars: await listEnvVariables(env.id).catch(() => [] as EnvVariable[]),
      })),
    ).then((next) => {
      if (cancelled) return;
      setBundles(next);
    });
    return () => {
      cancelled = true;
    };
  }, [environments, variablesVersion]);

  return bundles;
}
