// Epic 50 consumer-mount shell — composes the 4 DocHeader card
// components shipped this loop into a single render surface.
//
// Pure presentational. The actual page consumer (PaneContainer /
// MarkdownEditor wrapping for `.md` tabs) collects the data via
// Tauri / stores and passes it through. This shell handles the
// compact-mode gating logic (only H1 + meta strip visible per
// Story 06) and the `onToggleCompact` flow that consumers wire to
// the workspace.toml persistence.

import { Box, Flex } from "@chakra-ui/react";

import {
  PreflightPills,
  type PreflightPillItem,
} from "@/components/blocks/preflight/PreflightPills";

import { DocHeaderAbstract } from "./DocHeaderAbstract";
import { DocHeaderActionRow } from "./DocHeaderActionRow";
import { DocHeaderCard } from "./DocHeaderCard";
import { DocHeaderMetaStrip } from "./DocHeaderMetaStrip";
import { TagColumn } from "./TagColumn";
import type { DocHeaderFrontmatter } from "./docheader-derive";
import type {
  AuthorInfo,
  BranchSummaryData,
  LastRunSummary,
} from "./docheader-meta";

export interface DocHeaderShellProps {
  // ── DocHeaderCard inputs ───────────────────────────────────────
  filePath: string;
  relativeFilePath?: string | null;
  frontmatter?: DocHeaderFrontmatter | null;
  firstHeading?: string | null;
  /** True when the card is collapsed to H1 + meta strip only.
   *  Driven by the consumer (reads from workspace.toml). */
  compact?: boolean;
  /** Click-on-title toggle. Consumer flips compact + persists. */
  onToggleCompact?: () => void;
  onBreadcrumbSelect?: (path: string) => void;
  /** When provided, the H1 becomes an editable input (Notion-mode).
   *  See DocHeaderCard for debounce + sync semantics. */
  onTitleSave?: (title: string) => void;
  /** When provided, the abstract paragraph becomes an editable inline
   *  textarea. Newlines collapse to spaces on commit. */
  onAbstractSave?: (abstract: string) => void;
  /** Vault-wide tag union, fed into the TagColumn autocomplete. */
  availableTags?: ReadonlyArray<string>;
  /** Add / remove a single tag — the consumer rebuilds the full
   *  `tags:` line via `updateFrontmatterTags`. */
  onAddTag?: (tag: string) => void;
  onRemoveTag?: (tag: string) => void;

  // ── Meta strip inputs ──────────────────────────────────────────
  author?: AuthorInfo | null;
  mtimeMs?: number | null;
  dirty?: boolean;
  branch?: BranchSummaryData | null;
  lastRun?: LastRunSummary | null;
  onSelectAuthor?: () => void;
  onSelectEdited?: () => void;
  onSelectBranch?: () => void;
  onSelectLastRun?: () => void;

  // ── Action row inputs ──────────────────────────────────────────
  onRunAll?: () => void;
  runAllBusy?: boolean;
  onShare?: () => void;
  onDuplicate?: () => void;
  onArchive?: () => void;
  onDelete?: () => void;

  // ── Pre-flight inputs ──────────────────────────────────────────
  preflightItems?: ReadonlyArray<PreflightPillItem>;
  preflightRechecking?: boolean;
  onPreflightFailureSelect?: (item: PreflightPillItem) => void;
  onPreflightRecheck?: () => void;
}

export function DocHeaderShell(props: DocHeaderShellProps) {
  const {
    filePath,
    relativeFilePath,
    frontmatter,
    firstHeading,
    compact,
    onToggleCompact,
    onBreadcrumbSelect,
    onTitleSave,
    onAbstractSave,
    availableTags,
    onAddTag,
    onRemoveTag,
    author,
    mtimeMs,
    dirty,
    branch,
    lastRun,
    onSelectAuthor,
    onSelectEdited,
    onSelectBranch,
    onSelectLastRun,
    onRunAll,
    runAllBusy,
    onShare,
    onDuplicate,
    onArchive,
    onDelete,
    preflightItems,
    preflightRechecking,
    onPreflightFailureSelect,
    onPreflightRecheck,
  } = props;

  const showActionRow = !compact;
  const showAbstract = !compact;
  const showPreflight = !compact && (preflightItems?.length ?? 0) > 0;
  const showTags =
    !compact &&
    (onAddTag !== undefined ||
      onRemoveTag !== undefined ||
      (frontmatter?.tags?.length ?? 0) > 0);

  return (
    <Box data-testid="docheader-shell" data-compact={compact || undefined}>
      <Box position="relative">
        <DocHeaderCard
          filePath={filePath}
          relativeFilePath={relativeFilePath}
          frontmatter={frontmatter}
          firstHeading={firstHeading}
          compact={compact}
          onTitleClick={onToggleCompact}
          onBreadcrumbSelect={onBreadcrumbSelect}
          onTitleSave={onTitleSave}
        />
        {showActionRow && (
          <Box
            data-testid="docheader-shell-action-row-slot"
            position="absolute"
            top={5}
            right={6}
          >
            <DocHeaderActionRow
              onRunAll={onRunAll}
              runAllBusy={runAllBusy}
              onShare={onShare}
              onDuplicate={onDuplicate}
              onArchive={onArchive}
              onDelete={onDelete}
            />
          </Box>
        )}
        <Flex px={6} pb={3} direction="column">
          <DocHeaderMetaStrip
            author={author}
            mtimeMs={mtimeMs}
            dirty={dirty}
            branch={branch}
            lastRun={lastRun}
            onSelectAuthor={onSelectAuthor}
            onSelectEdited={onSelectEdited}
            onSelectBranch={onSelectBranch}
            onSelectLastRun={onSelectLastRun}
          />
          {showAbstract && (
            <Box data-testid="docheader-shell-abstract-slot">
              <DocHeaderAbstract
                frontmatter={frontmatter ?? null}
                onAbstractSave={onAbstractSave}
              />
            </Box>
          )}
          {showTags && (
            <Box data-testid="docheader-shell-tags-slot" mt={2}>
              <TagColumn
                tags={frontmatter?.tags ?? []}
                availableTags={availableTags}
                onAddTag={onAddTag}
                onRemoveTag={onRemoveTag}
              />
            </Box>
          )}
          {showPreflight && preflightItems && (
            <Box data-testid="docheader-shell-preflight-slot">
              <PreflightPills
                items={preflightItems}
                rechecking={preflightRechecking}
                onSelectFailure={onPreflightFailureSelect}
                onRecheck={onPreflightRecheck}
              />
            </Box>
          )}
        </Flex>
      </Box>
    </Box>
  );
}
