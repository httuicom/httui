// V2 / cenário 4.5 — DocHeader inline, mounted as a CM6 block widget at
// the top of the document (no separate React layer). The inlineHeader
// prop carries the data the standalone shell used to read; the
// MarkdownEditor's CM6 extension creates a portal slot that this
// component fills via the same DocHeaderShell render path.
//
// Pre-V2 the DocHeader was a sibling React layer above the editor with
// its own scroll surface. It's now a single-scroll editor with the
// header inline, matching the Notion-style mockup. ConflictBanner
// stays outside the CM6 editor so it pushes the whole pane down on
// stale-on-disk.

import { useEffect, useMemo, useRef } from "react";
import { Box } from "@chakra-ui/react";

import { MarkdownEditor } from "@/components/editor/MarkdownEditor";
import type { InlineDocHeader } from "@/components/editor/DocHeaderWidgetPortal";
import { ConflictBanner } from "../ConflictBanner";
import type { BranchSummaryData } from "../docheader/docheader-meta";
import { useFileDocHeaderCompact } from "@/hooks/useFileDocHeaderCompact";
import { useFileMtime } from "@/hooks/useFileMtime";
import { useGitStatus } from "@/hooks/useGitStatus";

export interface DocHeaderedEditorProps {
  filePath: string;
  vaultPath: string;
  content: string;
  vimEnabled: boolean;
  showConflict: boolean;
  /** Whether the active tab has unsaved edits (drives the meta-strip
   * `· unsaved` suffix on the `Edited Xm ago` chip). PaneNode reads
   * this from `unsavedFiles.has(filePath)`. */
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

  // Refresh the mtime poll on the dirty → clean rising edge — this
  // means a save just succeeded (the auto-save path flips
  // `unsavedFiles` from true → false after `writeNote` resolves).
  // Without this, the meta strip would lag until the next focus
  // event arrives, leaving "Edited 2m ago · unsaved" stale on
  // screen post-save.
  const prevDirtyRef = useRef(dirty);
  useEffect(() => {
    if (prevDirtyRef.current && !dirty) {
      refreshMtime();
    }
    prevDirtyRef.current = dirty;
  }, [dirty, refreshMtime]);

  const branch = useMemo<BranchSummaryData | null>(() => {
    if (!gitStatus) return null;
    // Per-file `+N ~M` requires a future Tauri command (`git_diff_stat
    // _for_file`) — for now we only surface the branch name. The
    // BranchSummaryData shape allows zero counts; formatBranchSummary
    // omits them, so the chip just shows "Branch <name>".
    return {
      branch: gitStatus.branch,
      addedLines: 0,
      modifiedLines: 0,
    };
  }, [gitStatus]);

  // The frontmatter and the editable callbacks live inside
  // `DocHeaderWidgetPortal` now — it dispatches transactions directly
  // into CM6 and reads the parsed frontmatter off the StateField,
  // bypassing the non-reactive `editorContents` Map in the pane store.
  // This component just provides the ambient metadata (mtime / dirty /
  // branch / compact) that doesn't depend on doc content.
  const inlineHeader = useMemo<InlineDocHeader>(
    () => ({
      filePath,
      compact,
      onToggleCompact: () => {
        void setCompact(!compact);
      },
      mtimeMs: mtime,
      dirty,
      branch,
    }),
    [filePath, compact, setCompact, mtime, dirty, branch],
  );

  return (
    <Box
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
    </Box>
  );
}
