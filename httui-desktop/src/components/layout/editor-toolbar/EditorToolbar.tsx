// Editor per-tab toolbar — Epic 39 Story 03.
//
// 28px strip mounted above the CM6 editor (between any conflict
// banner and the editor itself). Shows:
//   • file path (last 2 segments, ellipsis on overflow)
//   • "edited Xm ago" derived from FS mtime + dirty flag
//   • block count (executable blocks; HTTP / DB / etc.)
//   • ⚡ auto-capture toggle (per-file persisted)
//   • + Add block menu (md / http / sql / mongo / ws / gql / sh)
//
// Pure presentational: takes everything as props. The persistence
// hook (`useFileAutoCapture`) and mtime poll wire later in PaneNode
// alongside the Epic 30a Story 01 BlockRegistry refactor that
// retires the MarkdownEditor.tsx coverage:exclude.

import { Box, HStack, Text, chakra } from "@chakra-ui/react";
import { LuZap } from "react-icons/lu";

import {
  AddBlockMenu,
  type BlockTemplate,
} from "@/components/layout/AddBlockMenu";

const ToggleButton = chakra("button");

export interface EditorToolbarProps {
  /** Absolute or vault-relative file path of the active tab. */
  filePath: string;
  /** Last-modified timestamp from FS. `null` until the first poll
   * resolves. */
  editedAt: Date | null;
  /** Whether the active tab has unsaved edits — used to suffix the
   * "edited Xm ago" label with "(unsaved)". */
  unsaved: boolean;
  /** Total executable blocks in the doc. */
  blockCount: number;
  /** Per-file auto-capture flag. */
  autoCapture: boolean;
  /** Toggle handler. */
  onAutoCaptureChange: (next: boolean) => void;
  /** Optional handler for the "+ Add block" menu. When omitted, the
   *  menu is hidden — convenient for the readonly diff viewer. */
  onAddBlock?: (template: BlockTemplate) => void;
}

/** Last 2 segments of a path, ellipsis on the leading side when
 * truncated. Used to keep the toolbar compact while keeping the
 * file's parent folder visible (eg. `auth/login.md`). */
export function shortenPath(filePath: string): string {
  const parts = filePath.split("/").filter(Boolean);
  if (parts.length <= 2) return parts.join("/");
  return parts.slice(-2).join("/");
}

/** Render a relative-time string like "2m ago" / "3h ago" /
 * "5d ago" / "just now". Resolution caps at days; older is "Apr 5". */
export function formatRelativeTime(date: Date | null, now = new Date()): string {
  if (!date) return "—";
  const ms = now.getTime() - date.getTime();
  if (ms < 30_000) return "just now";
  const minutes = Math.floor(ms / 60_000);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days}d ago`;
  return date.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });
}

export function EditorToolbar({
  filePath,
  editedAt,
  unsaved,
  blockCount,
  autoCapture,
  onAutoCaptureChange,
  onAddBlock,
}: EditorToolbarProps) {
  const editedLabel = unsaved
    ? `${formatRelativeTime(editedAt)} (unsaved)`
    : `edited ${formatRelativeTime(editedAt)}`;

  return (
    <HStack
      data-atom="editor-toolbar"
      h="28px"
      px={3}
      gap={4}
      bg="bg.subtle"
      borderBottomWidth="1px"
      borderBottomColor="border"
      fontSize="11px"
      fontFamily="mono"
      color="fg.muted"
      flexShrink={0}
    >
      <Text
        data-testid="editor-toolbar-path"
        color="fg"
        maxW="320px"
        overflow="hidden"
        textOverflow="ellipsis"
        whiteSpace="nowrap"
        title={filePath}
      >
        {shortenPath(filePath)}
      </Text>
      <Text data-testid="editor-toolbar-edited">{editedLabel}</Text>
      <Text data-testid="editor-toolbar-blocks">
        {blockCount === 1 ? "1 block" : `${blockCount} blocks`}
      </Text>

      <Box flex={1} />

      <ToggleButton
        type="button"
        data-testid="editor-toolbar-autocapture"
        data-active={autoCapture ? "true" : "false"}
        aria-pressed={autoCapture}
        aria-label={
          autoCapture ? "Disable auto-capture" : "Enable auto-capture"
        }
        onClick={() => onAutoCaptureChange(!autoCapture)}
        h="20px"
        px={2}
        gap={1}
        display="inline-flex"
        alignItems="center"
        borderRadius="3px"
        bg={autoCapture ? "brand.subtle" : "transparent"}
        color={autoCapture ? "brand.fg" : "fg.subtle"}
        fontFamily="mono"
        fontSize="10px"
        fontWeight={500}
        cursor="pointer"
        _hover={
          autoCapture
            ? { bg: "brand.subtle", opacity: 0.85 }
            : { bg: "bg.muted", color: "fg.muted" }
        }
      >
        <LuZap size={10} />
        auto-capture
      </ToggleButton>

      {onAddBlock && (
        <AddBlockMenu
          onInsert={onAddBlock}
          ariaLabel="Add block to document"
        />
      )}
    </HStack>
  );
}
