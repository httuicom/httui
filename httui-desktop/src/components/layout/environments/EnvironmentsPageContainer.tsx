// Smart wrapper around <EnvironmentsPage /> (V5 cenário 6). Owns:
// - env list load + per-env varCount adapter into EnvironmentSummary
// - file watcher refresh on `config-changed` (category "environment")
// - activate-env wiring (delegates to useEnvironmentStore)
//
// Mirrors VariablesPageContainer / ConnectionsPageContainer:
// presentational page stays prop-driven, data + IPC live here.

import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import { useEnvironmentStore } from "@/stores/environment";
import { listEnvVariables, type Environment } from "@/lib/tauri/commands";

import { EnvironmentsPage } from "./EnvironmentsPage";
import type { EnvironmentSummary } from "./envs-meta";

/** Map a backend Environment + its variable count into the
 * EnvironmentSummary the EnvironmentsPage expects.
 *
 * `filename` defaults to `${name}.toml` because the backend does not
 * surface the actual on-disk path (the `.local.toml` distinction
 * lands when the backend exposes it). `isPersonal` therefore stays
 * false for now — the personal/committed split is display-only and
 * can be enriched without breaking page consumers. */
export function envToSummary(env: Environment, varCount: number): EnvironmentSummary {
  return {
    name: env.name,
    filename: `${env.name}.toml`,
    varCount,
    connectionsUsedCount: env.connections_used?.length ?? 0,
    isActive: env.is_active,
    isPersonal: false,
    isTemporary: Boolean(env.temporary),
    description: env.description ?? undefined,
  };
}

interface EnvironmentsPageContainerProps {
  /** No-op for V5 cenário 6 (defer create form to a follow-up). */
  onCreateNew?: () => void;
}

export function EnvironmentsPageContainer({
  onCreateNew,
}: EnvironmentsPageContainerProps) {
  const environments = useEnvironmentStore((s) => s.environments);
  const refreshEnvs = useEnvironmentStore((s) => s.refresh);
  const switchEnvironment = useEnvironmentStore((s) => s.switchEnvironment);
  const variablesVersion = useEnvironmentStore((s) => s.variablesVersion);

  const [summaries, setSummaries] = useState<EnvironmentSummary[]>([]);

  useEffect(() => {
    void refreshEnvs();
  }, [refreshEnvs]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;
    void (async () => {
      const fn = await listen<{ category: string }>("config-changed", (e) => {
        if (e.payload.category === "environment") {
          void refreshEnvs();
        }
      });
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    })();
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [refreshEnvs]);

  // Load varCount per env in parallel, then assemble summaries.
  useEffect(() => {
    let cancelled = false;
    if (environments.length === 0) {
      setSummaries([]);
      return;
    }
    void Promise.all(
      environments.map(async (env) => {
        const vars = await listEnvVariables(env.id).catch(() => []);
        return envToSummary(env, vars.length);
      }),
    ).then((next) => {
      if (cancelled) return;
      setSummaries(next);
    });
    return () => {
      cancelled = true;
    };
  }, [environments, variablesVersion]);

  const handleActivate = useCallback(
    (filename: string) => {
      // EnvironmentSummary identity is the filename — strip the
      // `.toml` (or `.local.toml`) suffix and find the matching env.
      const base = filename.replace(/\.local\.toml$/i, "").replace(/\.toml$/i, "");
      const target = environments.find((e) => e.name === base);
      if (!target) return;
      void switchEnvironment(target.id);
    },
    [environments, switchEnvironment],
  );

  return (
    <EnvironmentsPage
      envs={summaries}
      onActivate={handleActivate}
      onCreateNew={onCreateNew}
    />
  );
}
