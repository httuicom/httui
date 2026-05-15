// V10 — smart wrapper around <GitPanel />. Owns git data fetching,
// IPC dispatch, and the active tab; the presentational panel stays
// prop-driven and trivially testable. Mirrors
// ConnectionsPageContainer (V4) — the singleton-pane-tab pattern.
//
// Cenário 1 scope: Status tab (status header + working tree) + Log /
// Audit tabs reading `gitLog`. Status polls every 2s via
// `useGitStatus`; a save (pane `saveSignal` bump) forces an
// immediate refresh so the file list reflects the edit without
// waiting for the next poll tick.

import { useCallback, useEffect, useState } from "react";

import { useGitStatus } from "@/hooks/useGitStatus";
import { gitLog, type CommitInfo, type GitFileChange } from "@/lib/tauri/git";
import { usePaneStore } from "@/stores/pane";
import { useWorkspaceStore } from "@/stores/workspace";

import { GitPanel, type GitPanelTab } from "./GitPanel";

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

  const reloadLog = useCallback(async () => {
    if (!vaultPath) {
      setCommits([]);
      return;
    }
    try {
      const list = await gitLog(vaultPath, LOG_LIMIT);
      setCommits(list);
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

  const handleSelectFile = useCallback((file: GitFileChange) => {
    setSelectedFilePath((prev) => (prev === file.path ? null : file.path));
  }, []);

  const handleSelectCommit = useCallback((commit: CommitInfo) => {
    setSelectedCommitSha((prev) =>
      prev === commit.sha ? null : commit.sha,
    );
  }, []);

  return (
    <GitPanel
      status={status}
      commits={commits}
      activeTab={activeTab}
      onSelectTab={setActiveTab}
      selectedFilePath={selectedFilePath}
      selectedCommitSha={selectedCommitSha}
      onSelectFile={handleSelectFile}
      onSelectCommit={handleSelectCommit}
    />
  );
}
