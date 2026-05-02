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

import { useCallback, useEffect, useMemo, useRef } from "react";
import { Box } from "@chakra-ui/react";

import { MarkdownEditor } from "@/components/editor/MarkdownEditor";
import type { InlineDocHeader } from "@/components/editor/DocHeaderWidgetPortal";
import { ConflictBanner } from "../ConflictBanner";
import type { BranchSummaryData } from "../docheader/docheader-meta";
import { useFileDocHeaderCompact } from "@/hooks/useFileDocHeaderCompact";
import { useFileMtime } from "@/hooks/useFileMtime";
import { useGitStatus } from "@/hooks/useGitStatus";
import { extractFrontmatter } from "@/lib/blocks/extract-frontmatter-tags";
import { updateFrontmatterTitle } from "@/lib/blocks/update-frontmatter";
import type { DocHeaderFrontmatter } from "../docheader/docheader-derive";

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

  // Parse frontmatter inline (synchronous TS port of the Rust slice-1
  // schema — title + abstract + tags only). Re-runs every keystroke,
  // but the parser short-circuits on `---\n` absence so the common
  // body-edit case is a single string-prefix check. The Rust
  // `parse_frontmatter` stays the authoritative parser on the vault-
  // walker path; this is the per-edit synchronous counterpart.
  const frontmatter = useMemo<DocHeaderFrontmatter | null>(() => {
    const fm = extractFrontmatter(content);
    if (
      fm.title === undefined &&
      fm.abstract === undefined &&
      fm.tags.length === 0
    ) {
      // No frontmatter at all → null lets the card fall back through
      // first-heading → filename. Distinct from "fenced but empty"
      // which still renders the card chrome.
      return null;
    }
    return {
      title: fm.title,
      abstract: fm.abstract,
      tags: fm.tags,
    };
  }, [content]);

  // Latest content + onChange refs so the title-save callback below can
  // be stable across body keystrokes — without this the callback rebuilds
  // on every render (content is in scope), which the editable input
  // already tolerates via its onSaveRef but produces unnecessary portal
  // re-renders. Refs flatten that to one render per title commit.
  const contentRef = useRef(content);
  const onChangeRef = useRef(onChange);
  useEffect(() => {
    contentRef.current = content;
    onChangeRef.current = onChange;
  });

  const onTitleSave = useCallback((title: string) => {
    const next = updateFrontmatterTitle(contentRef.current, title);
    if (next === contentRef.current) return;
    onChangeRef.current(next);
  }, []);

  const inlineHeader = useMemo<InlineDocHeader>(
    () => ({
      filePath,
      frontmatter,
      compact,
      onToggleCompact: () => {
        void setCompact(!compact);
      },
      mtimeMs: mtime,
      dirty,
      branch,
      onTitleSave,
    }),
    [
      filePath,
      frontmatter,
      compact,
      setCompact,
      mtime,
      dirty,
      branch,
      onTitleSave,
    ],
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
