// V10 — smart wrapper around <GitPanel />. Owns git data fetching,
// IPC dispatch, the active tab, the commit-form state, and the log
// filter; the presentational panel stays prop-driven and trivially
// testable. Mirrors ConnectionsPageContainer (V4) — the
// singleton-pane-tab pattern.
//
// Cenários covered here:
//  1. Status/Log/Audit tabs, 2s poll + save-signal refresh.
//  2. Stage toggle, commit form, working-diff preview, commit.
//  3. Log list filter (author = in-memory, path = backend re-fetch)
//     + commit-diff inspector on the Log tab.

import { useCallback, useEffect, useMemo, useState } from "react";

import { useGitCommit } from "@/hooks/useGitCommit";
import { useGitConflictResolve } from "@/hooks/useGitConflictResolve";
import { useGitRemotes } from "@/hooks/useGitRemotes";
import { useGitStatus } from "@/hooks/useGitStatus";
import { writeNote } from "@/lib/tauri/commands";
import {
  gitConflictVersions,
  gitDiff,
  gitFetch,
  gitPull,
  gitPush,
  stagePath,
  unstagePath,
  type CommitInfo,
  type ConflictVersions,
  type GitFileChange,
} from "@/lib/tauri/git";
import { useGitStore } from "@/stores/git";
import { usePaneStore } from "@/stores/pane";
import { useWorkspaceStore } from "@/stores/workspace";

import { ShareMenu } from "@/components/layout/ShareMenu";

import { GitPanel, type GitPanelTab } from "./GitPanel";
import { labelFileStatus, partitionFileChanges } from "./git-derive";
import type { SyncOp } from "./GitSyncButtons";
import {
  filterCommitsByAuthor,
  parsePathFilter,
  type LogFilterState,
} from "./git-log-filter";

interface GitPanelContainerProps {
  onNavigateFile?: (filePath: string) => void;
}

export function GitPanelContainer(_props: GitPanelContainerProps) {
  const vaultPath = useWorkspaceStore((s) => s.vaultPath);
  const saveSignal = usePaneStore((s) => s.saveSignal);

  const { status, refresh: refreshStatus } = useGitStatus(vaultPath);

  const commits = useGitStore((s) => s.commits);
  const reloadLog = useGitStore((s) => s.reloadLog);
  const [activeTab, setActiveTab] = useState<GitPanelTab>("status");
  const [selectedFilePath, setSelectedFilePath] = useState<string | null>(null);
  const [selectedCommitSha, setSelectedCommitSha] = useState<string | null>(
    null,
  );
  const commitMessage = useGitStore((s) => s.commitMessage);
  const setCommitMessage = useGitStore((s) => s.setCommitMessage);
  const [commitAmend, setCommitAmend] = useState(false);
  const { commit, committing } = useGitCommit(vaultPath);
  const [diff, setDiff] = useState<string | null | undefined>(undefined);
  const [diffSubject, setDiffSubject] = useState<string | null>(null);
  const [diffShortSha, setDiffShortSha] = useState<string | null>(null);
  const [logFilter, setLogFilter] = useState<LogFilterState>({
    mode: "author",
    query: "",
  });
  const [syncInFlight, setSyncInFlight] = useState<SyncOp | null>(null);
  const [upstreamPrompt, setUpstreamPrompt] = useState<{
    branch: string;
    remote: string;
  } | null>(null);

  const [resolver, setResolver] = useState<{
    path: string;
    versions: ConflictVersions;
  } | null>(null);

  const { remotes } = useGitRemotes(vaultPath);
  const hasRemote = remotes.length > 0;

  const {
    busy: conflictBusy,
    acceptOurs,
    acceptTheirs,
  } = useGitConflictResolve(vaultPath);

  const conflicts = useMemo(
    () =>
      (status?.changed ?? [])
        .filter((c) => labelFileStatus(c) === "conflicted")
        .map((c) => c.path),
    [status],
  );

  // Path mode filters server-side (CommitInfo carries no paths);
  // author mode filters the in-memory list. Deriving pathFilter
  // outside the callback keeps author keystrokes from re-fetching.
  const pathFilter =
    logFilter.mode === "path" ? parsePathFilter(logFilter.query) : null;

  const refreshLog = useCallback(
    () => reloadLog(pathFilter ?? undefined),
    [reloadLog, pathFilter],
  );

  useEffect(() => {
    void refreshLog();
  }, [refreshLog]);

  // A save just landed — reflect it immediately instead of waiting
  // for the 2s status poll. saveSignal is bumped by `notifySaved`.
  useEffect(() => {
    if (saveSignal === 0) return;
    refreshStatus();
    void refreshLog();
  }, [saveSignal, refreshStatus, refreshLog]);

  const handleToggleStage = useCallback(
    async (file: GitFileChange) => {
      if (!vaultPath) return;
      try {
        if (file.staged) {
          await unstagePath(vaultPath, file.path);
        } else {
          await stagePath(vaultPath, file.path);
        }
      } finally {
        refreshStatus();
      }
    },
    [vaultPath, refreshStatus],
  );

  const handleSelectFile = useCallback(
    async (file: GitFileChange) => {
      const next = selectedFilePath === file.path ? null : file.path;
      setSelectedFilePath(next);
      setSelectedCommitSha(null);
      if (next === null || !vaultPath) {
        setDiff(undefined);
        return;
      }
      setDiff(null);
      setDiffShortSha(null);
      setDiffSubject("Working tree changes");
      try {
        setDiff(await gitDiff(vaultPath));
      } catch {
        setDiff("");
      }
    },
    [selectedFilePath, vaultPath],
  );

  const handleSelectCommit = useCallback(
    async (commit: CommitInfo) => {
      const next = selectedCommitSha === commit.sha ? null : commit.sha;
      setSelectedCommitSha(next);
      setSelectedFilePath(null);
      if (next === null || !vaultPath) {
        setDiff(undefined);
        return;
      }
      setDiff(null);
      setDiffShortSha(commit.short_sha);
      setDiffSubject(commit.subject);
      try {
        setDiff(await gitDiff(vaultPath, commit.sha));
      } catch {
        setDiff("");
      }
    },
    [selectedCommitSha, vaultPath],
  );

  const handleCommit = useCallback(
    async (input: { message: string; amend: boolean }) => {
      await commit(input);
      setCommitAmend(false);
      setDiff(undefined);
      setSelectedFilePath(null);
      await refreshLog();
    },
    [commit, refreshLog],
  );

  const runSync = useCallback(
    async (op: SyncOp, fn: () => Promise<unknown>, touchesTree: boolean) => {
      if (!vaultPath || syncInFlight) return;
      setSyncInFlight(op);
      try {
        await fn();
        refreshStatus();
        await refreshLog();
        if (touchesTree) {
          await useWorkspaceStore.getState().refreshFileTree(vaultPath);
        }
      } catch {
        // git stderr is surfaced by the status poll / a future toast;
        // the sync row just returns to idle so the user can retry.
      } finally {
        setSyncInFlight(null);
      }
    },
    [vaultPath, syncInFlight, refreshStatus, refreshLog],
  );

  const handleFetch = useCallback(
    () => runSync("fetch", () => gitFetch(vaultPath!), false),
    [runSync, vaultPath],
  );

  const handlePull = useCallback(
    () => runSync("pull", () => gitPull(vaultPath!), true),
    [runSync, vaultPath],
  );

  const doPush = useCallback(
    (setUpstream: boolean) => {
      const branch = status?.branch ?? null;
      return runSync(
        "push",
        () =>
          setUpstream && branch
            ? gitPush(vaultPath!, "origin", branch, true)
            : gitPush(vaultPath!),
        false,
      );
    },
    [runSync, vaultPath, status],
  );

  const handlePush = useCallback(() => {
    if (status && status.upstream === null && status.branch) {
      setUpstreamPrompt({ branch: status.branch, remote: "origin" });
      return;
    }
    void doPush(false);
  }, [status, doPush]);

  const handleConfirmSetUpstream = useCallback(() => {
    setUpstreamPrompt(null);
    void doPush(true);
  }, [doPush]);

  const handleCancelSetUpstream = useCallback(
    () => setUpstreamPrompt(null),
    [],
  );

  const handleOpenConflict = useCallback(
    async (path: string) => {
      if (!vaultPath) return;
      try {
        const versions = await gitConflictVersions(vaultPath, path);
        setResolver({ path, versions });
      } catch {
        // Path stopped being unmerged between render and click —
        // the status poll will drop it from the banner shortly.
      }
    },
    [vaultPath],
  );

  const afterConflictMutation = useCallback(() => {
    refreshStatus();
    void refreshLog();
  }, [refreshStatus, refreshLog]);

  const handleAcceptYours = useCallback(
    async (path: string) => {
      await acceptOurs(path);
      afterConflictMutation();
    },
    [acceptOurs, afterConflictMutation],
  );

  const handleAcceptTheirs = useCallback(
    async (path: string) => {
      await acceptTheirs(path);
      afterConflictMutation();
    },
    [acceptTheirs, afterConflictMutation],
  );

  const handleResolveMerged = useCallback(
    async (path: string, merged: string) => {
      if (!vaultPath) return;
      try {
        await writeNote(vaultPath, path, merged);
        await stagePath(vaultPath, path);
      } finally {
        setResolver(null);
        afterConflictMutation();
      }
    },
    [vaultPath, afterConflictMutation],
  );

  const handleCancelResolver = useCallback(() => setResolver(null), []);

  const stagedCount = status
    ? partitionFileChanges(status.changed).staged.length
    : 0;

  const visibleCommits = useMemo(
    () =>
      logFilter.mode === "author"
        ? filterCommitsByAuthor(commits, logFilter.query)
        : commits,
    [commits, logFilter],
  );

  return (
    <GitPanel
      status={status}
      commits={visibleCommits}
      activeTab={activeTab}
      onSelectTab={setActiveTab}
      selectedFilePath={selectedFilePath}
      selectedCommitSha={selectedCommitSha}
      onToggleStage={handleToggleStage}
      onSelectFile={handleSelectFile}
      onSelectCommit={handleSelectCommit}
      stagedCount={stagedCount}
      commitMessage={commitMessage}
      commitAmend={commitAmend}
      committing={committing}
      onCommitMessageChange={setCommitMessage}
      onCommitAmendChange={setCommitAmend}
      onCommit={handleCommit}
      diff={diff}
      diffShortSha={diffShortSha}
      diffSubject={diffSubject}
      logFilter={logFilter}
      onLogFilterChange={setLogFilter}
      syncInFlight={syncInFlight}
      hasRemote={hasRemote}
      onFetch={handleFetch}
      onPull={handlePull}
      onPush={handlePush}
      upstreamPrompt={upstreamPrompt}
      onConfirmSetUpstream={handleConfirmSetUpstream}
      onCancelSetUpstream={handleCancelSetUpstream}
      conflicts={conflicts}
      conflictBusy={conflictBusy}
      onOpenConflict={handleOpenConflict}
      onAcceptYours={handleAcceptYours}
      onAcceptTheirs={handleAcceptTheirs}
      resolver={resolver}
      onResolveMerged={handleResolveMerged}
      onCancelResolver={handleCancelResolver}
      toolbarExtra={<ShareMenu vaultPath={vaultPath} variant="toolbar" />}
    />
  );
}
