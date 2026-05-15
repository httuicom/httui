// V10 — smart wrapper around <GitPanel />. Owns git data fetching,
// IPC dispatch, the active tab, and the commit-form state; the
// presentational panel stays prop-driven and trivially testable.
// Mirrors ConnectionsPageContainer (V4) — the singleton-pane-tab
// pattern.
//
// Cenários covered here:
//  1. Status/Log/Audit tabs, 2s poll + save-signal refresh.
//  2. Stage toggle (stage/unstage), commit form, working-diff
//     preview on file select, commit → clear + refresh.

import { useCallback, useEffect, useState } from "react";

import { useGitStatus } from "@/hooks/useGitStatus";
import {
  gitCommit,
  gitDiff,
  gitLog,
  stagePath,
  unstagePath,
  type CommitInfo,
  type GitFileChange,
} from "@/lib/tauri/git";
import { usePaneStore } from "@/stores/pane";
import { useWorkspaceStore } from "@/stores/workspace";

import { GitPanel, type GitPanelTab } from "./GitPanel";
import { partitionFileChanges } from "./git-derive";

const LOG_LIMIT = 50;

interface GitPanelContainerProps {
  onNavigateFile?: (filePath: string) => void;
}

export function GitPanelContainer(_props: GitPanelContainerProps) {
  const vaultPath = useWorkspaceStore((s) => s.vaultPath);
  const saveSignal = usePaneStore((s) => s.saveSignal);

  const { status, refresh: refreshStatus } = useGitStatus(vaultPath);

  const [commits, setCommits] = useState<CommitInfo[]>([]);
  const [activeTab, setActiveTab] = useState<GitPanelTab>("status");
  const [selectedFilePath, setSelectedFilePath] = useState<string | null>(
    null,
  );
  const [selectedCommitSha, setSelectedCommitSha] = useState<string | null>(
    null,
  );
  const [commitMessage, setCommitMessage] = useState("");
  const [commitAmend, setCommitAmend] = useState(false);
  const [committing, setCommitting] = useState(false);
  const [diff, setDiff] = useState<string | null | undefined>(undefined);
  const [diffSubject, setDiffSubject] = useState<string | null>(null);

  const reloadLog = useCallback(async () => {
    if (!vaultPath) {
      setCommits([]);
      return;
    }
    try {
      setCommits(await gitLog(vaultPath, LOG_LIMIT));
    } catch {
      // Transient (not a repo yet, IPC dead) — the status poll
      // surfaces real errors; the log list just stays empty.
      setCommits([]);
    }
  }, [vaultPath]);

  useEffect(() => {
    void reloadLog();
  }, [reloadLog]);

  // A save just landed — reflect it immediately instead of waiting
  // for the 2s status poll. saveSignal is bumped by `notifySaved`.
  useEffect(() => {
    if (saveSignal === 0) return;
    refreshStatus();
    void reloadLog();
  }, [saveSignal, refreshStatus, reloadLog]);

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
      setDiffSubject("Working tree changes");
      try {
        setDiff(await gitDiff(vaultPath));
      } catch {
        setDiff("");
      }
    },
    [selectedFilePath, vaultPath],
  );

  // Commit selection is visual-only here (Log list highlight). The
  // commit-diff fetch lands in cenário 3 when the Log tab gains its
  // diff panel — adding the fetch now would be untested dead code.
  const handleSelectCommit = useCallback((commit: CommitInfo) => {
    setSelectedCommitSha((prev) =>
      prev === commit.sha ? null : commit.sha,
    );
  }, []);

  const handleCommit = useCallback(
    async (input: { message: string; amend: boolean }) => {
      if (!vaultPath || committing) return;
      setCommitting(true);
      try {
        await gitCommit(vaultPath, input.message, input.amend);
        setCommitMessage("");
        setCommitAmend(false);
        setDiff(undefined);
        setSelectedFilePath(null);
        refreshStatus();
        await reloadLog();
      } finally {
        setCommitting(false);
      }
    },
    [vaultPath, committing, refreshStatus, reloadLog],
  );

  const stagedCount = status
    ? partitionFileChanges(status.changed).staged.length
    : 0;

  return (
    <GitPanel
      status={status}
      commits={commits}
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
      diffSubject={diffSubject}
    />
  );
}
