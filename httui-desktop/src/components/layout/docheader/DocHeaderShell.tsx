// consumer-mount shell — composes the 4 DocHeader card
// components shipped this loop into a single render surface.
//
// Pure presentational. The actual page consumer (PaneContainer /
// MarkdownEditor wrapping for `.md` tabs) collects the data via
// Tauri / stores and passes it through. This shell handles the
// compact-mode gating logic (only H1 + meta strip visible in
// compact mode) and the `onToggleCompact` flow that consumers wire
// to the workspace.toml persistence.

import { Box, Flex } from "@chakra-ui/react";

import {
  PreflightPills,
  type PreflightPillItem,
} from "@/components/blocks/preflight/PreflightPills";

import type { TaskItem } from "@/lib/blocks/task-item";

import { DocHeaderAbstract } from "./DocHeaderAbstract";
import { DocHeaderActionRow } from "./DocHeaderActionRow";
import { DocHeaderCard } from "./DocHeaderCard";
import { DocHeaderChecklist } from "./DocHeaderChecklist";
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
  /** Click-on-title toggle. Consumer flips compact + persists. Used by
   * static consumers (diff viewer, snapshots) only
   *  prefers `onTitleNavigateToBody` for the inline editor since
   *  toggling compact is a no-op in editable mode. */
  onToggleCompact?: () => void;
  /** click on the (static) H1 navigates to the first
   *  body line and focuses the cursor there. Wired by the inline editor
   *  consumer (`DocHeaderWidgetPortal`) to `returnFocusToBody`. Takes
   *  precedence over `onToggleCompact` when both are provided. */
  onTitleNavigateToBody?: () => void;
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
  /** Checklist save callback. Receives the full new list on every
   *  edit (toggle / text change / add / remove). The list is persisted
   *  to the `tasks:` YAML key (renamed from `preflight:` in V6
   * */
  onChecklistSave?: (items: TaskItem[]) => void;

  // ── Meta strip inputs ──────────────────────────────────────────
  author?: AuthorInfo | null;
  mtimeMs?: number | null;
  dirty?: boolean;
  branch?: BranchSummaryData | null;
  lastRun?: LastRunSummary | null;
  blockCount?: number;
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
  /** builder callbacks. When wired, the pill row
   *  surfaces a `+ Add check` button and click-to-edit on existing
   *  pills. The consumer mutates the frontmatter via the
   *  `updateFrontmatterPreflightChecks` writer. */
  onAddPreflightCheck?: (
    check: import("@/lib/blocks/preflight-checks").PreflightCheck,
  ) => void;
  onEditPreflightCheck?: (
    idx: number,
    next: import("@/lib/blocks/preflight-checks").PreflightCheck,
  ) => void;
  onRemovePreflightCheck?: (idx: number) => void;
}

export function DocHeaderShell(props: DocHeaderShellProps) {
  const {
    filePath,
    relativeFilePath,
    frontmatter,
    firstHeading,
    compact,
    onToggleCompact,
    onTitleNavigateToBody,
    onBreadcrumbSelect,
    onTitleSave,
    onAbstractSave,
    availableTags,
    onAddTag,
    onRemoveTag,
    onChecklistSave,
    author,
    mtimeMs,
    dirty,
    branch,
    lastRun,
    blockCount,
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
    onAddPreflightCheck,
    onEditPreflightCheck,
    onRemovePreflightCheck,
  } = props;

  // in editable mode (any save callback provided),
  // compact is forcibly disabled — the legacy "click H1 to toggle"
  // affordance went away when the title became an editable input, so
  // a stale `docheader_compact: true` in workspace.toml would otherwise
  // hide the abstract / tags / checklist with no way for the user to
  // get them back. Static consumers (diff viewer, snapshots) keep the
  // compact behavior intact.
  const editableMode =
    onTitleSave !== undefined ||
    onAbstractSave !== undefined ||
    onAddTag !== undefined ||
    onRemoveTag !== undefined ||
    onChecklistSave !== undefined;
  const effectiveCompact = editableMode ? false : compact;

  const showActionRow = !effectiveCompact;
  const showAbstract = !effectiveCompact;
  const showPreflight =
    !effectiveCompact &&
    ((preflightItems?.length ?? 0) > 0 || onAddPreflightCheck !== undefined);
  const showTags =
    !effectiveCompact &&
    (onAddTag !== undefined ||
      onRemoveTag !== undefined ||
      (frontmatter?.tags?.length ?? 0) > 0);
  const showChecklist =
    !effectiveCompact &&
    (onChecklistSave !== undefined || (frontmatter?.tasks?.length ?? 0) > 0);

  return (
    <Box
      data-testid="docheader-shell"
      data-compact={effectiveCompact || undefined}
    >
      <Box position="relative">
        <DocHeaderCard
          filePath={filePath}
          relativeFilePath={relativeFilePath}
          frontmatter={frontmatter}
          firstHeading={firstHeading}
          compact={effectiveCompact}
          onTitleClick={onTitleNavigateToBody ?? onToggleCompact}
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
            blockCount={blockCount}
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
          {(showChecklist || showTags) && (
            <Flex
              data-testid="docheader-shell-meta-row"
              direction="row"
              gap={6}
              mt={3}
              align="flex-start"
            >
              {showChecklist ? (
                <Box
                  data-testid="docheader-shell-checklist-slot"
                  flex={1}
                  minW={0}
                >
                  <DocHeaderChecklist
                    items={frontmatter?.tasks ?? []}
                    onChecklistSave={onChecklistSave}
                  />
                </Box>
              ) : (
                // Reserve the column even when there's no checklist so
                // the tag column stays right-aligned.
                <Box flex={1} />
              )}
              {showTags && (
                <Box
                  data-testid="docheader-shell-tags-slot"
                  width="180px"
                  flexShrink={0}
                >
                  <TagColumn
                    tags={frontmatter?.tags ?? []}
                    availableTags={availableTags}
                    onAddTag={onAddTag}
                    onRemoveTag={onRemoveTag}
                  />
                </Box>
              )}
            </Flex>
          )}
          {showPreflight && (
            <Box data-testid="docheader-shell-preflight-slot">
              <PreflightPills
                items={preflightItems ?? []}
                rechecking={preflightRechecking}
                onSelectFailure={onPreflightFailureSelect}
                onRecheck={onPreflightRecheck}
                onAddCheck={onAddPreflightCheck}
                onEditCheck={onEditPreflightCheck}
                onRemoveCheck={onRemovePreflightCheck}
              />
            </Box>
          )}
        </Flex>
      </Box>
    </Box>
  );
}
