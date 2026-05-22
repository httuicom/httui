import { useState } from "react";
import { Flex, Text, chakra } from "@chakra-ui/react";

import { gravatarUrl } from "@/lib/avatars/gravatar";

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
  /** `frontmatter.owner` from the parser. Hidden when
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
      {author && <AuthorChip author={author} onClick={onSelectAuthor} />}
      {trimmedOwner && (
        <Chip
          testId="docheader-meta-owner"
          tone="muted"
          onClick={
            onSelectOwner ? () => onSelectOwner(trimmedOwner) : undefined
          }
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
      {/* Hide last-run chip when the doc currently has no blocks —
          stale `block_run_history` rows from earlier sessions would
          otherwise surface confusing "1 block · 1 failed" copy on
          a doc that has no executable blocks at all. */}
      {lastRun && (typeof blockCount !== "number" || blockCount > 0) && (
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
      bg="bg.muted"
      color={palette.color}
      flexShrink={0}
      title={title}
      onClick={onClick}
      cursor={onClick ? "pointer" : undefined}
      _hover={onClick ? { bg: "bg.emphasized" } : undefined}
    >
      {children}
    </Text>
  );
}

function chipPalette(tone: ChipTone): { color: string } {
  switch (tone) {
    case "ok":
      return { color: "brand.fg" };
    case "warn":
      return { color: "warn" };
    case "fail":
      return { color: "error" };
    default:
      return { color: "fg.muted" };
  }
}

function authorDisplayName(info: AuthorInfo): string {
  const name = info.name?.trim();
  if (name && name.length > 0) return name;
  const email = info.email?.trim();
  if (email && email.length > 0) {
    const local = email.split("@")[0];
    if (local) return local;
  }
  return "?";
}

/**
 * Pick a stable accent hue from the author's name/email so distinct
 * authors get visually distinct avatar circles even when the underlying
 * `--chakra-colors-accent` token resolves to the same value.
 */
function authorHue(info: AuthorInfo): number {
  const seed = (info.name ?? info.email ?? "?").toLowerCase();
  let hash = 0;
  for (let i = 0; i < seed.length; i += 1) {
    hash = (hash * 31 + seed.charCodeAt(i)) | 0;
  }
  return Math.abs(hash) % 360;
}

function AuthorChip({
  author,
  onClick,
}: {
  author: AuthorInfo;
  onClick?: () => void;
}) {
  const initials = authorInitialsFromFirstCommit(author);
  const display = authorDisplayName(author);
  const hue = authorHue(author);
  const title = author.name
    ? `${author.name}${author.email ? ` <${author.email}>` : ""}`
    : "Unknown author";
  // Gravatar with `d=404` returns a real 404 when the email has no
  // associated account — `onError` fires and we render the colored
  // initials circle instead. Skip the request entirely when there's
  // no email at all.
  const gravatar = gravatarUrl(author.email, { size: 20 });
  const [gravatarOk, setGravatarOk] = useState<boolean>(gravatar !== null);
  return (
    <Flex
      as={onClick ? "button" : "span"}
      data-testid="docheader-meta-author"
      align="center"
      gap={2}
      title={title}
      onClick={onClick}
      cursor={onClick ? "pointer" : undefined}
      flexShrink={0}
    >
      {gravatar && gravatarOk ? (
        <chakra.img
          data-testid="docheader-meta-author-avatar"
          src={gravatar}
          alt=""
          w="20px"
          h="20px"
          borderRadius="999px"
          flexShrink={0}
          onError={() => setGravatarOk(false)}
        />
      ) : (
        <Flex
          data-testid="docheader-meta-author-fallback"
          align="center"
          justify="center"
          w="20px"
          h="20px"
          borderRadius="999px"
          flexShrink={0}
          css={{
            backgroundColor: `oklch(0.62 0.14 ${hue})`,
            color: "#ffffff",
            fontFamily: "var(--chakra-fonts-mono)",
            fontSize: "9px",
            fontWeight: 700,
            letterSpacing: "0.02em",
          }}
        >
          {initials}
        </Flex>
      )}
      <Text
        fontFamily="mono"
        fontSize="11px"
        color="fg.muted"
        _hover={onClick ? { color: "fg" } : undefined}
      >
        {display}
      </Text>
    </Flex>
  );
}
