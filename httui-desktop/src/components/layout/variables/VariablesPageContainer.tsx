// Smart wrapper around <VariablesPage /> (V5). Owns:
// - cross-env variable load + merge (per-key map of values per env)
// - vault-grep `entries` per key (Tauri `grep_var_uses`); count
//   derives from length, full list feeds the detail panel
// - detail panel composition: VariableDetailContent with per-env
//   value rows, secret reveal/edit, used-in-blocks list, and the
//   is_secret toggle wired to setVariable
// - file watcher refresh on `config-changed` (category "environment")
//
// Mirrors the V4 ConnectionsPageContainer pattern: presentational
// page stays prop-driven, data + IPC live here.

import { useCallback, useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import { useEnvironmentStore } from "@/stores/environment";
import { useSessionOverrideStore } from "@/stores/sessionOverride";
import { useWorkspaceStore } from "@/stores/workspace";
import {
  listEnvVariables,
  resolveEnvVariables,
  type Environment,
  type EnvVariable,
} from "@/lib/tauri/commands";
import { grepVarUses, type VarUseEntry } from "@/lib/tauri/var-uses";

import { UsedInBlocksList } from "./UsedInBlocksList";
import { VariableDetailContent } from "./VariableDetailContent";
import { VariablesPage } from "./VariablesPage";
import type { VariableRow } from "./variable-derive";

interface VariablesPageContainerProps {
  onNavigateFile?: (filePath: string) => void;
}

interface EnvVarsBundle {
  env: Environment;
  vars: EnvVariable[];
}

/** Merge per-env variable lists into one row per key. Values map is
 * keyed by env *name* so the page's `envColumnNames` (also names)
 * resolves directly. Secret rows mask via `isSecret = true`. */
export function mergeCrossEnvVariables(
  bundles: ReadonlyArray<EnvVarsBundle>,
): VariableRow[] {
  const byKey = new Map<string, VariableRow>();
  for (const { env, vars } of bundles) {
    for (const v of vars) {
      const existing = byKey.get(v.key);
      if (existing) {
        existing.values = { ...existing.values, [env.name]: v.value };
        if (v.is_secret) existing.isSecret = true;
      } else {
        byKey.set(v.key, {
          key: v.key,
          // V5 cenário 1: scope inference is `workspace` for every
          // env-defined var. Personal/captured discrimination ships
          // alongside the per-env meta backend (later cenário).
          scope: "workspace",
          isSecret: Boolean(v.is_secret),
          values: { [env.name]: v.value },
          usesCount: 0,
        });
      }
    }
  }
  return [...byKey.values()];
}

export function VariablesPageContainer({
  onNavigateFile,
}: VariablesPageContainerProps) {
  const vaultPath = useWorkspaceStore((s) => s.vaultPath);
  const environments = useEnvironmentStore((s) => s.environments);
  const activeEnvironment = useEnvironmentStore((s) => s.activeEnvironment);
  const refreshEnvs = useEnvironmentStore((s) => s.refresh);
  const setVariable = useEnvironmentStore((s) => s.setVariable);
  const variablesVersion = useEnvironmentStore((s) => s.variablesVersion);
  const overrides = useSessionOverrideStore((s) => s.overrides);
  const setOverride = useSessionOverrideStore((s) => s.setOverride);
  const clearOverride = useSessionOverrideStore((s) => s.clearOverride);

  const [rows, setRows] = useState<VariableRow[]>([]);
  const [usesEntriesByKey, setUsesEntriesByKey] = useState<
    Record<string, VarUseEntry[]>
  >({});
  const [selectedKey, setSelectedKey] = useState<string | null>(null);

  // Initial env load — store does not auto-refresh on mount.
  useEffect(() => {
    void refreshEnvs();
  }, [refreshEnvs]);

  // External `envs/*.toml` edits via the file watcher (Epic 11).
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

  // Cross-env merge whenever the env list changes or a setVariable
  // bumps `variablesVersion`.
  useEffect(() => {
    let cancelled = false;
    if (environments.length === 0) {
      setRows([]);
      return;
    }
    void Promise.all(
      environments.map(async (env) => ({
        env,
        vars: await listEnvVariables(env.id).catch(() => [] as EnvVariable[]),
      })),
    ).then((bundles) => {
      if (cancelled) return;
      const merged = mergeCrossEnvVariables(bundles);
      const annotated = merged.map((r) => ({
        ...r,
        usesCount: usesEntriesByKey[r.key]?.length ?? 0,
      }));
      setRows(annotated);
    });
    return () => {
      cancelled = true;
    };
  }, [environments, variablesVersion, usesEntriesByKey]);

  // One-shot vault grep per key. Cheap (regex over *.md) and the
  // result is invariant to env changes — refetch only when the key
  // set changes or vault switches.
  const keysSignature = useMemo(
    () => [...new Set(rows.map((r) => r.key))].sort().join(""),
    [rows],
  );
  useEffect(() => {
    if (!vaultPath || rows.length === 0) return;
    let cancelled = false;
    const keys = [...new Set(rows.map((r) => r.key))];
    void Promise.all(
      keys.map(async (key) => ({
        key,
        entries: await grepVarUses(vaultPath, key).catch(
          () => [] as VarUseEntry[],
        ),
      })),
    ).then((results) => {
      if (cancelled) return;
      const next: Record<string, VarUseEntry[]> = {};
      for (const { key, entries } of results) next[key] = entries;
      setUsesEntriesByKey(next);
    });
    return () => {
      cancelled = true;
    };
    // keysSignature collapses identical key sets so we don't refetch
    // when env-only props change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [vaultPath, keysSignature]);

  const envColumnNames = useMemo(
    () => environments.map((e) => e.name),
    [environments],
  );
  const envByName = useMemo(() => {
    const m = new Map<string, Environment>();
    for (const e of environments) m.set(e.name, e);
    return m;
  }, [environments]);

  const handleSelectKey = useCallback((key: string) => {
    setSelectedKey(key);
  }, []);

  const selectedRow = useMemo(
    () =>
      selectedKey ? rows.find((r) => r.key === selectedKey) ?? null : null,
    [selectedKey, rows],
  );

  const fetchSecret = useCallback(
    async (envName: string): Promise<string | undefined> => {
      const e = envByName.get(envName);
      if (!e || !selectedKey) return undefined;
      const map = await resolveEnvVariables(e.id);
      return map[selectedKey];
    },
    [envByName, selectedKey],
  );

  const handleCommitValue = useCallback(
    async (envName: string, next: string) => {
      const e = envByName.get(envName);
      if (!e || !selectedRow) return;
      await setVariable(e.id, selectedRow.key, next, selectedRow.isSecret);
    },
    [envByName, selectedRow, setVariable],
  );

  const overridesByEnv = useMemo(() => {
    if (!selectedRow) return {};
    const out: Record<string, string> = {};
    for (const env of envColumnNames) {
      const v = overrides[env]?.[selectedRow.key];
      if (v !== undefined) out[env] = v;
    }
    return out;
  }, [overrides, selectedRow, envColumnNames]);

  const detailSlot = selectedRow ? (
    <VariableDetailContent
      row={selectedRow}
      envNames={envColumnNames}
      fetchSecret={fetchSecret}
      onCommitValue={handleCommitValue}
      onSetOverride={(env, next) => setOverride(env, selectedRow.key, next)}
      onClearOverride={(env) => clearOverride(env, selectedRow.key)}
      overridesByEnv={overridesByEnv}
      usedInBlocksSlot={
        <UsedInBlocksList
          entries={usesEntriesByKey[selectedRow.key]}
          onJump={(filePath) => onNavigateFile?.(filePath)}
        />
      }
    />
  ) : null;

  return (
    <VariablesPage
      envColumnNames={envColumnNames}
      activeEnvName={activeEnvironment?.name}
      rows={rows}
      selectedKey={selectedKey}
      onSelectKey={handleSelectKey}
      detailSlot={detailSlot}
    />
  );
}
