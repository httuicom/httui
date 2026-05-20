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
import { useConfigChangeRefresh } from "@/hooks/useConfigChangeRefresh";

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

import { NewVariableForm } from "./NewVariableForm";
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
          // scope inference is `workspace` for every
          // env-defined var. Personal/captured discrimination ships
          // alongside the per-env meta backend (later).
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
  const [creating, setCreating] = useState(false);

  // Initial env load — store does not auto-refresh on mount.
  useEffect(() => {
    void refreshEnvs();
  }, [refreshEnvs]);

  // External `envs/*.toml` edits via the file watcher.
  useConfigChangeRefresh("environment", refreshEnvs);

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
      selectedKey ? (rows.find((r) => r.key === selectedKey) ?? null) : null,
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

  // flip the is_secret flag for every env that defines
  // this key. Prompts in both directions: promotion moves the
  // cleartext into the keychain, demotion writes it back to the TOML.
  // For demotion we resolve via `resolveEnvVariables` because
  // `row.values[env]` is masked once the var is secret.
  const handleToggleSecret = useCallback(
    async (next: boolean) => {
      if (!selectedRow) return;
      const message = next
        ? `Move "${selectedRow.key}" to the keychain? Value(s) will be removed from envs/*.toml.`
        : `Remove "${selectedRow.key}" from the keychain? Value(s) will be written as plaintext to envs/*.toml.`;
      if (!window.confirm(message)) return;
      for (const [envName, currentValue] of Object.entries(
        selectedRow.values,
      )) {
        if (currentValue === undefined) continue;
        const e = envByName.get(envName);
        if (!e) continue;
        let valueToWrite = currentValue;
        if (selectedRow.isSecret && !next) {
          const resolved = await resolveEnvVariables(e.id).catch(
            (): Record<string, string> => ({}),
          );
          valueToWrite = resolved[selectedRow.key] ?? "";
        }
        await setVariable(e.id, selectedRow.key, valueToWrite, next);
      }
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

  const existingNames = useMemo(() => rows.map((r) => r.key), [rows]);

  const handleCreateSubmit = useCallback(
    async (payload: {
      name: string;
      value: string;
      isSecret: boolean;
      env: string;
    }) => {
      const e = envByName.get(payload.env);
      if (!e) return;
      await setVariable(e.id, payload.name, payload.value, payload.isSecret);
      setCreating(false);
      setSelectedKey(payload.name);
    },
    [envByName, setVariable],
  );

  const inlineFormSlot =
    creating && activeEnvironment ? (
      <NewVariableForm
        activeEnv={activeEnvironment.name}
        existingNames={existingNames}
        onSubmit={handleCreateSubmit}
        onCancel={() => setCreating(false)}
      />
    ) : null;

  const detailSlot = selectedRow ? (
    <VariableDetailContent
      row={selectedRow}
      envNames={envColumnNames}
      fetchSecret={fetchSecret}
      onCommitValue={handleCommitValue}
      onSetOverride={(env, next) => setOverride(env, selectedRow.key, next)}
      onClearOverride={(env) => clearOverride(env, selectedRow.key)}
      overridesByEnv={overridesByEnv}
      onToggleSecret={handleToggleSecret}
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
      onCreateNew={activeEnvironment ? () => setCreating(true) : undefined}
      detailSlot={detailSlot}
      inlineFormSlot={inlineFormSlot}
    />
  );
}
