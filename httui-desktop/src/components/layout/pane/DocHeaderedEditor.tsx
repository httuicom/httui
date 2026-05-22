import { useEffect, useMemo, useRef, useState } from "react";
import { Box } from "@chakra-ui/react";

import { MarkdownEditor } from "@/components/editor/MarkdownEditor";
import type { InlineDocHeader } from "@/components/editor/DocHeaderWidgetPortal";
import { ConflictBanner } from "../ConflictBanner";
import type {
  AuthorInfo,
  BranchSummaryData,
  LastRunSummary,
} from "../docheader/docheader-meta";
import { useFileDocHeaderCompact } from "@/hooks/useFileDocHeaderCompact";
import { useFileMtime } from "@/hooks/useFileMtime";
import { useFilePreflight } from "@/hooks/useFilePreflight";
import { useGitStatus } from "@/hooks/useGitStatus";
import { useRunAllPreflightGate } from "@/hooks/useRunAllPreflightGate";
import { gitFirstCommitAuthor } from "@/lib/tauri/git";
import { blockHistoryLastRunSummary } from "@/lib/tauri/block-history";

export interface DocHeaderedEditorProps {
  filePath: string;
  vaultPath: string;
  content: string;
  vimEnabled: boolean;
  showConflict: boolean;
  /** Whether the active tab has unsaved edits — drives the `· unsaved` suffix. */
  dirty: boolean;
  onConflictReload: () => void;
  onConflictKeep: () => void;
  onChange: (content: string) => void;
  onNavigateFile?: (filePath: string) => void;
}

export function DocHeaderedEditor({
  filePath,
  vaultPath,
  content,
  vimEnabled,
  showConflict,
  dirty,
  onConflictReload,
  onConflictKeep,
  onChange,
  onNavigateFile,
}: DocHeaderedEditorProps) {
  const { compact, setCompact } = useFileDocHeaderCompact(vaultPath, filePath);
  const { mtime, refresh: refreshMtime } = useFileMtime(vaultPath, filePath);
  const { status: gitStatus } = useGitStatus(vaultPath);
  const {
    items: preflightItems,
    rechecking: preflightRechecking,
    recheck: preflightRecheck,
  } = useFilePreflight({ filePath, vaultPath });

  // Refresh mtime on the dirty→clean rising edge (save just resolved).
  // Without this the meta strip would show "Edited 2m ago · unsaved" stale.
  const prevDirtyRef = useRef(dirty);
  useEffect(() => {
    if (prevDirtyRef.current && !dirty) {
      refreshMtime();
    }
    prevDirtyRef.current = dirty;
  }, [dirty, refreshMtime]);

  const branch = useMemo<BranchSummaryData | null>(() => {
    if (!gitStatus) return null;
    // Per-file +N ~M requires a future `git_diff_stat_for_file` command.
    // Zero counts are omitted by formatBranchSummary, so chip shows "Branch <name>".
    return {
      branch: gitStatus.branch,
      addedLines: 0,
      modifiedLines: 0,
    };
  }, [gitStatus]);

  // Vault-relative path for breadcrumb + git lookup (POSIX only — vault
  // layer normalises on open).
  const relativeFilePath = useMemo<string | null>(() => {
    if (!filePath) return null;
    if (vaultPath && filePath.startsWith(vaultPath + "/")) {
      return filePath.slice(vaultPath.length + 1);
    }
    return filePath;
  }, [filePath, vaultPath]);

  // First-commit author (follows renames). `null` while loading or untracked.
  const [author, setAuthor] = useState<AuthorInfo | null>(null);
  useEffect(() => {
    let cancelled = false;
    if (!relativeFilePath || !vaultPath) {
      setAuthor(null);
      return;
    }
    gitFirstCommitAuthor(vaultPath, relativeFilePath)
      .then((commit) => {
        if (cancelled) return;
        setAuthor(
          commit
            ? { name: commit.author_name, email: commit.author_email }
            : null,
        );
      })
      .catch(() => {
        if (!cancelled) setAuthor(null);
      });
    return () => {
      cancelled = true;
    };
  }, [vaultPath, relativeFilePath]);

  // Aggregated last-run summary from `block_run_history`. 5s session window lives in Rust.
  const [lastRun, setLastRun] = useState<LastRunSummary | null>(null);
  useEffect(() => {
    let cancelled = false;
    if (!filePath) {
      setLastRun(null);
      return;
    }
    blockHistoryLastRunSummary(filePath)
      .then((raw) => {
        if (cancelled) return;
        setLastRun({
          ranAt: raw.ran_at,
          blockCount: raw.block_count,
          failedCount: raw.failed_count,
        });
      })
      .catch(() => {
        if (!cancelled) setLastRun(null);
      });
    return () => {
      cancelled = true;
    };
  }, [filePath]);

  const { trigger: triggerRunAll, dialog: runAllDialog } =
    useRunAllPreflightGate({
      items: preflightItems,
      onRunAll: (decision) => {
        // eslint-disable-next-line no-console
        console.info(
          `[run-all] ${filePath} — failed=${decision.failedCount} skipped=${decision.skippedCount} note=${decision.auditNote ?? "(none)"}`,
        );
      },
    });

  // ⌘⇧R triggers the Run-all gate. Multi-pane safety: only the pane
  // containing the active focus target reacts.
  const rootRef = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (!mod || !e.shiftKey) return;
      if (e.key !== "R" && e.key !== "r") return;
      // Only run when this pane has focus (or the whole app is
      // focusless and this is the only mounted pane).
      const root = rootRef.current;
      const focus = document.activeElement;
      if (root && focus && !root.contains(focus)) return;
      e.preventDefault();
      triggerRunAll();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [triggerRunAll]);

  const inlineHeader = useMemo<InlineDocHeader>(
    () => ({
      filePath,
      relativeFilePath,
      compact,
      onToggleCompact: () => {
        void setCompact(!compact);
      },
      mtimeMs: mtime,
      dirty,
      branch,
      author,
      lastRun,
      preflightItems,
      preflightRechecking,
      onPreflightRecheck: preflightRecheck,
      // `onRunAll` intentionally absent — ▶ Run all was dropped from the
      // workspace chrome; the gate triggers only via ⌘⇧R.
    }),
    [
      filePath,
      relativeFilePath,
      compact,
      setCompact,
      mtime,
      dirty,
      branch,
      author,
      lastRun,
      preflightItems,
      preflightRechecking,
      preflightRecheck,
    ],
  );

  return (
    <Box
      ref={rootRef}
      data-testid="doc-headered-editor"
      flex={1}
      overflow="hidden"
      display="flex"
      flexDirection="column"
    >
      {showConflict && (
        <ConflictBanner
          filePath={filePath}
          onReload={onConflictReload}
          onKeep={onConflictKeep}
        />
      )}
      <Box flex={1} overflow="hidden">
        <MarkdownEditor
          content={content}
          onChange={onChange}
          filePath={filePath}
          vimEnabled={vimEnabled}
          onNavigateFile={onNavigateFile}
          inlineHeader={inlineHeader}
        />
      </Box>
      {runAllDialog}
    </Box>
  );
}
