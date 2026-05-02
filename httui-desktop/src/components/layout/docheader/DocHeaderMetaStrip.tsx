// Epic 50 Story 03 — meta strip below the H1.
//
// Compact horizontal row of chips. Pure presentational; consumer
// fetches the data via Tauri (git author + git status + run history)
// and passes the assembled props. Each chip is a button when an
// `onSelect*` handler is supplied so the consumer can navigate to
// the relevant panel (Git / History / etc.).

import { Flex, Text } from "@chakra-ui/react";

import {
  authorInitialsFromFirstCommit,
  formatBranchSummary,
  formatEditedTime,
  formatLastRun,
  lastRunTone,
  type AuthorInfo,
  type BranchSummaryData,
  type LastRunSummary,
} from "./docheader-meta";

export interface DocHeaderMetaStripProps {
  author?: AuthorInfo | null;
  /** Filesystem mtime in ms; null while loading or for unsaved files. */
  mtimeMs?: number | null;
  /** True when the editor has unsaved changes. */
  dirty?: boolean;
  branch?: BranchSummaryData | null;
  lastRun?: LastRunSummary | null;
  /** `frontmatter.owner` from the Story 01 parser. Hidden when
   *  null / undefined / empty so consumer mounts unconditionally. */
  owner?: string | null;
  /** Number of executable fenced blocks in the body. Hidden when
   *  zero / undefined. */
  blockCount?: number;
  onSelectAuthor?: () => void;
  onSelectEdited?: () => void;
  onSelectBranch?: () => void;
  onSelectLastRun?: () => void;
  onSelectOwner?: (owner: string) => void;
}

export function DocHeaderMetaStrip({
  author,
  mtimeMs,
  dirty,
  branch,
  lastRun,
  owner,
  blockCount,
  onSelectAuthor,
  onSelectEdited,
  onSelectBranch,
  onSelectLastRun,
  onSelectOwner,
}: DocHeaderMetaStripProps) {
  const trimmedOwner = owner?.trim();
  return (
    <Flex
      data-testid="docheader-meta-strip"
      align="center"
      gap={2}
      mt={2}
      flexWrap="wrap"
    >
      {author && (
        <Chip
          testId="docheader-meta-author"
          tone="muted"
          onClick={onSelectAuthor}
          title={
            author.name
              ? `${author.name}${author.email ? ` <${author.email}>` : ""}`
              : "Unknown author"
          }
        >
          {authorInitialsFromFirstCommit(author)}
        </Chip>
      )}
      {trimmedOwner && (
        <Chip
          testId="docheader-meta-owner"
          tone="muted"
          onClick={onSelectOwner ? () => onSelectOwner(trimmedOwner) : undefined}
          title={`owner: ${trimmedOwner}`}
        >
          @{trimmedOwner}
        </Chip>
      )}
      {mtimeMs !== undefined && (
        <Chip
          testId="docheader-meta-edited"
          tone={dirty ? "warn" : "muted"}
          onClick={onSelectEdited}
        >
          {formatEditedTime(mtimeMs ?? null, !!dirty)}
        </Chip>
      )}
      {typeof blockCount === "number" && blockCount > 0 && (
        <Chip testId="docheader-meta-blocks" tone="muted">
          {blockCount === 1 ? "1 block" : `${blockCount} blocks`}
        </Chip>
      )}
      {branch && (
        <Chip
          testId="docheader-meta-branch"
          tone="muted"
          onClick={onSelectBranch}
        >
          {formatBranchSummary(branch)}
        </Chip>
      )}
      {lastRun && (
        <Chip
          testId="docheader-meta-last-run"
          tone={lastRunTone(lastRun)}
          onClick={onSelectLastRun}
        >
          {formatLastRun(lastRun)}
        </Chip>
      )}
    </Flex>
  );
}

type ChipTone = "muted" | "warn" | "ok" | "fail";

function Chip({
  testId,
  tone,
  title,
  onClick,
  children,
}: {
  testId: string;
  tone: ChipTone;
  title?: string;
  onClick?: () => void;
  children: React.ReactNode;
}) {
  const palette = chipPalette(tone);
  return (
    <Text
      as={onClick ? "button" : "span"}
      data-testid={testId}
      data-tone={tone}
      fontFamily="mono"
      fontSize="11px"
      px={2}
      py={1}
      borderRadius="3px"
      bg="bg.2"
      color={palette.color}
      flexShrink={0}
      title={title}
      onClick={onClick}
      cursor={onClick ? "pointer" : undefined}
      _hover={onClick ? { bg: "bg.3" } : undefined}
    >
      {children}
    </Text>
  );
}

function chipPalette(tone: ChipTone): { color: string } {
  switch (tone) {
    case "ok":
      return { color: "accent" };
    case "warn":
      return { color: "warn" };
    case "fail":
      return { color: "error" };
    default:
      return { color: "fg.2" };
  }
}
