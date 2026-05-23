import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { listen } from "@tauri-apps/api/event";

import { useEnvironmentStore } from "@/stores/environment";
import { type Environment } from "@/lib/tauri/commands";

import {
  CloneEnvironmentForm,
  type CloneEnvironmentPayload,
} from "./CloneEnvironmentForm";
import { DeleteEnvironmentConfirm } from "./DeleteEnvironmentConfirm";
import { EnvironmentsPage } from "./EnvironmentsPage";
import type { EnvironmentSummary } from "./envs-meta";
import { envNameFromFilename } from "./envs-meta";
import {
  NewEnvironmentForm,
  type NewEnvironmentPayload,
} from "./NewEnvironmentForm";
import {
  RenameEnvironmentForm,
  type RenameEnvironmentPayload,
} from "./RenameEnvironmentForm";

/** Map a backend Environment + its variable count into EnvironmentSummary.
 * `isPersonal` is always false until the backend exposes the on-disk path. */
export function envToSummary(
  env: Environment,
  varCount: number,
): EnvironmentSummary {
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
  /** When omitted the container surfaces its own inline create form. */
  onCreateNew?: () => void;
}

export function EnvironmentsPageContainer({
  onCreateNew,
}: EnvironmentsPageContainerProps) {
  const environments = useEnvironmentStore((s) => s.environments);
  const activeEnvironment = useEnvironmentStore((s) => s.activeEnvironment);
  const refreshEnvs = useEnvironmentStore((s) => s.refresh);
  const switchEnvironment = useEnvironmentStore((s) => s.switchEnvironment);
  const createEnvironment = useEnvironmentStore((s) => s.createEnvironment);
  const duplicateEnvironment = useEnvironmentStore(
    (s) => s.duplicateEnvironment,
  );
  const renameEnvironment = useEnvironmentStore((s) => s.renameEnvironment);
  const deleteEnvironment = useEnvironmentStore((s) => s.deleteEnvironment);

  // FLIP: capture old pill rect before switchEnvironment fires; animate in useLayoutEffect.
  const oldPillRectRef = useRef<DOMRect | null>(null);
  const prevActiveNameRef = useRef<string | null>(
    activeEnvironment?.name ?? null,
  );

  const [cloning, setCloning] = useState<{
    filename: string;
    name: string;
  } | null>(null);
  const [renaming, setRenaming] = useState<EnvironmentSummary | null>(null);
  const [deleting, setDeleting] = useState<EnvironmentSummary | null>(null);
  const [creatingEnv, setCreatingEnv] = useState(false);

  // Refresh on mount + on external `envs/*.toml` edits (backend
  // `config-changed` category "environment").
  useConfigSyncedResource("environment", refreshEnvs);

  // Shared cross-env fan-out (audit 05 §A.3 / backlog S2). The async
  // load lives in the hook; the page-specific derivation below stays
  // a synchronous memo — same output, same cadence as the old effect.
  const bundles = useCrossEnvVariables();

  useEffect(() => {
    let cancelled = false;
    if (environments.length === 0) {
      setSummaries([]);
      setSecretCounts({});
      return;
    }
    void Promise.all(
      environments.map(async (env) => {
        const vars = await listEnvVariables(env.id).catch(() => []);
        const secrets = vars.filter((v) => v.is_secret).length;
        return {
          summary: envToSummary(env, vars.length),
          secrets,
        };
      }),
    ).then((rows) => {
      if (cancelled) return;
      setSummaries(rows.map((r) => r.summary));
      setSecretCounts(
        Object.fromEntries(
          rows.map(({ summary, secrets }) => [summary.filename, secrets]),
        ),
      );
    });
    return () => {
      cancelled = true;
    };
  }, [environments, variablesVersion]);

  const envByFilename = useMemo(() => {
    const m = new Map<string, Environment>();
    for (const e of environments) {
      m.set(`${e.name}.toml`, e);
    }
    return m;
  }, [environments]);

  const handleActivate = useCallback(
    (filename: string) => {
      const base = envNameFromFilename(filename);
      const target = environments.find((e) => e.name === base);
      if (!target) return;
      const oldPill = document.querySelector<HTMLElement>(
        '[data-env-active-pill="true"]',
      );
      oldPillRectRef.current = oldPill?.getBoundingClientRect() ?? null;
      void switchEnvironment(target.id);
    },
    [environments, switchEnvironment],
  );

  // FLIP: only animate after summaries (not just activeEnvironment) updates,
  // so the new pill is already in its real DOM position before we read its rect.
  useLayoutEffect(() => {
    const nextName =
      summaries.find((s) => s.isActive)?.name ??
      activeEnvironment?.name ??
      null;
    if (nextName === prevActiveNameRef.current) return;
    prevActiveNameRef.current = nextName;
    const oldRect = oldPillRectRef.current;
    oldPillRectRef.current = null;
    if (!oldRect || !nextName) return;
    const newPill = document.querySelector<HTMLElement>(
      '[data-env-active-pill="true"]',
    );
    if (!newPill) return;
    const newRect = newPill.getBoundingClientRect();
    const dx = oldRect.left - newRect.left;
    const dy = oldRect.top - newRect.top;
    if (dx === 0 && dy === 0) return;
    const reduced = window.matchMedia(
      "(prefers-reduced-motion: reduce)",
    ).matches;
    if (reduced) return;
    newPill.style.transition = "none";
    newPill.style.transform = `translate(${dx}px, ${dy}px)`;
    requestAnimationFrame(() => {
      newPill.style.transition =
        "transform 360ms cubic-bezier(0.22, 1, 0.36, 1)";
      newPill.style.transform = "translate(0, 0)";
    });
  }, [summaries, activeEnvironment]);

  const handleRequestClone = useCallback(
    (filename: string) => {
      const target = envByFilename.get(filename);
      if (!target) return;
      setCloning({ filename, name: target.name });
      setRenaming(null);
      setDeleting(null);
    },
    [envByFilename],
  );

  const handleRequestRename = useCallback(
    (filename: string) => {
      const summary = summaries.find((s) => s.filename === filename);
      if (!summary) return;
      setRenaming(summary);
      setCloning(null);
      setDeleting(null);
    },
    [summaries],
  );

  const handleRequestDelete = useCallback(
    (filename: string) => {
      const summary = summaries.find((s) => s.filename === filename);
      if (!summary) return;
      setDeleting(summary);
      setCloning(null);
      setRenaming(null);
    },
    [summaries],
  );

  // `copyConnectionsUsed` / `markTemporary` / `markPersonal` are UI-only
  // until the backend exposes those parameters on duplicate_environment.
  const handleCloneSubmit = useCallback(
    async (payload: CloneEnvironmentPayload) => {
      const source = envByFilename.get(payload.sourceFilename);
      if (!source) return;
      await duplicateEnvironment(source.id, payload.name);
      setCloning(null);
    },
    [duplicateEnvironment, envByFilename],
  );

  const handleRenameSubmit = useCallback(
    async (payload: RenameEnvironmentPayload) => {
      const source = envByFilename.get(payload.sourceFilename);
      if (!source) return;
      await renameEnvironment(source.id, payload.newName);
      setRenaming(null);
    },
    [envByFilename, renameEnvironment],
  );

  const handleDeleteConfirm = useCallback(
    async (filename: string) => {
      const target = envByFilename.get(filename);
      if (!target) return;
      await deleteEnvironment(target.id);
      setDeleting(null);
    },
    [deleteEnvironment, envByFilename],
  );

  const handleCreateNewSubmit = useCallback(
    async (payload: NewEnvironmentPayload) => {
      await createEnvironment(payload.name);
      setCreatingEnv(false);
    },
    [createEnvironment],
  );

  const activeForm = cloning ? (
    <CloneEnvironmentForm
      sourceFilename={cloning.filename}
      sourceName={cloning.name}
      existingFilenames={summaries.map((s) => s.filename)}
      onSubmit={handleCloneSubmit}
      onCancel={() => setCloning(null)}
    />
  ) : renaming ? (
    <RenameEnvironmentForm
      env={renaming}
      existingFilenames={summaries.map((s) => s.filename)}
      onSubmit={handleRenameSubmit}
      onCancel={() => setRenaming(null)}
    />
  ) : deleting ? (
    <DeleteEnvironmentConfirm
      env={deleting}
      secretCount={secretCounts[deleting.filename] ?? 0}
      onConfirm={handleDeleteConfirm}
      onCancel={() => setDeleting(null)}
    />
  ) : null;

  const activeFilename =
    cloning?.filename ?? renaming?.filename ?? deleting?.filename ?? null;
  const closeAllForms = () => {
    setCloning(null);
    setRenaming(null);
    setDeleting(null);
  };

  const inlineFormSlot = creatingEnv ? (
    <NewEnvironmentForm
      existingFilenames={summaries.map((s) => s.filename)}
      onSubmit={handleCreateNewSubmit}
      onCancel={() => setCreatingEnv(false)}
    />
  ) : null;

  return (
    <EnvironmentsPage
      envs={summaries}
      onActivate={handleActivate}
      onCreateNew={onCreateNew ?? (() => setCreatingEnv(true))}
      onClone={handleRequestClone}
      onRename={handleRequestRename}
      onDelete={handleRequestDelete}
      inlineFormSlot={inlineFormSlot}
      anchoredForm={activeForm}
      anchoredFilename={activeFilename}
      onCloseAnchoredForm={closeAllForms}
    />
  );
}
