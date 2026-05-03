// Smart wrapper around <EnvironmentsPage /> (V5 cenário 6). Owns:
// - env list load + per-env varCount adapter into EnvironmentSummary
// - file watcher refresh on `config-changed` (category "environment")
// - activate-env wiring (delegates to useEnvironmentStore)
//
// Mirrors VariablesPageContainer / ConnectionsPageContainer:
// presentational page stays prop-driven, data + IPC live here.

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
import { listEnvVariables, type Environment } from "@/lib/tauri/commands";

import {
  CloneEnvironmentForm,
  type CloneEnvironmentPayload,
} from "./CloneEnvironmentForm";
import { DeleteEnvironmentConfirm } from "./DeleteEnvironmentConfirm";
import { EnvironmentsPage } from "./EnvironmentsPage";
import type { EnvironmentSummary } from "./envs-meta";
import { envNameFromFilename } from "./envs-meta";
import {
  RenameEnvironmentForm,
  type RenameEnvironmentPayload,
} from "./RenameEnvironmentForm";

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
  const activeEnvironment = useEnvironmentStore((s) => s.activeEnvironment);
  const refreshEnvs = useEnvironmentStore((s) => s.refresh);
  const switchEnvironment = useEnvironmentStore((s) => s.switchEnvironment);
  const duplicateEnvironment = useEnvironmentStore(
    (s) => s.duplicateEnvironment,
  );
  const renameEnvironment = useEnvironmentStore((s) => s.renameEnvironment);
  const deleteEnvironment = useEnvironmentStore((s) => s.deleteEnvironment);
  const variablesVersion = useEnvironmentStore((s) => s.variablesVersion);

  // FLIP swap of the ACTIVE pill across cards: capture the old
  // pill's bounding rect before switchEnvironment fires, then in
  // useLayoutEffect (post-commit, pre-paint) translate the new pill
  // back to the old position and animate it to its real spot.
  const oldPillRectRef = useRef<DOMRect | null>(null);
  const prevActiveNameRef = useRef<string | null>(
    activeEnvironment?.name ?? null,
  );

  const [summaries, setSummaries] = useState<EnvironmentSummary[]>([]);
  const [cloning, setCloning] = useState<{
    filename: string;
    name: string;
  } | null>(null);
  const [renaming, setRenaming] = useState<EnvironmentSummary | null>(null);
  const [deleting, setDeleting] = useState<EnvironmentSummary | null>(null);
  const [secretCounts, setSecretCounts] = useState<Record<string, number>>({});

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

  // Load varCount + secretCount per env in parallel, then assemble
  // summaries.
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
      // Capture the current pill's position BEFORE the state flip so
      // the FLIP effect below can animate from old → new.
      const oldPill = document.querySelector<HTMLElement>(
        '[data-env-active-pill="true"]',
      );
      oldPillRectRef.current = oldPill?.getBoundingClientRect() ?? null;
      void switchEnvironment(target.id);
    },
    [environments, switchEnvironment],
  );

  // FLIP — runs after every commit. Fires only when the active env
  // actually changed AND the local `summaries` already reflects the
  // new sort (so the new pill is mounted in its real DOM position).
  // Depending on `summaries` (not `activeEnvironment`) ensures we
  // only animate after the per-var refetch has flushed the cards.
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

  // V5 cenário 7 — backend `duplicate_environment` only accepts
  // (source_id, new_name) and copies plain vars by default. The form's
  // `copyConnectionsUsed` / `markTemporary` / `markPersonal`
  // checkboxes stay UI-only until the backend grows the parameters.
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

  return (
    <EnvironmentsPage
      envs={summaries}
      onActivate={handleActivate}
      onCreateNew={onCreateNew}
      onClone={handleRequestClone}
      onRename={handleRequestRename}
      onDelete={handleRequestDelete}
      anchoredForm={activeForm}
      anchoredFilename={activeFilename}
      onCloseAnchoredForm={closeAllForms}
    />
  );
}
