// Epic 48 Story 01 — middle section: working-tree file list grouped
// into Staged / Unstaged / Untracked.
//
// Pure presentational. Story 01 ships:
//   - layout (groups, headers, per-row checkbox + status icon + path)
//   - data attributes / callbacks for staging + per-row click
//
// Story 02 wires `onToggleStage(file)` to `stage_path`/`unstage_path`
// Tauri commands and `onSelect(file)` to the diff side-panel — Story
// 01 is purely the UI surface.

import { Box, Flex, Text } from "@chakra-ui/react";

import { Checkbox } from "@/components/ui/checkbox";
import type { GitFileChange } from "@/lib/tauri/git";

import { labelFileStatus, partitionFileChanges } from "./git-derive";

export interface GitFileListProps {
  changed: ReadonlyArray<GitFileChange>;
  /** Path of the row currently selected for diff display, if any. */
  selectedPath?: string | null;
  onToggleStage?: (file: GitFileChange) => void;
  onSelect?: (file: GitFileChange) => void;
}

export function GitFileList({
  changed,
  selectedPath,
  onToggleStage,
  onSelect,
}: GitFileListProps) {
  if (changed.length === 0) {
    return (
      <Text
        data-testid="git-file-list-empty"
        fontSize="11px"
        color="fg.subtle"
        px={3}
        py={4}
      >
        Working tree clean.
      </Text>
    );
  }

  const { staged, unstaged, untracked } = partitionFileChanges(changed);

  return (
    <Box data-testid="git-file-list" data-count={changed.length}>
      {staged.length > 0 && (
        <Group
          testId="git-file-list-staged"
          label="Staged"
          count={staged.length}
        >
          {staged.map((f) => (
            <Row
              key={f.path}
              file={f}
              checked
              selected={selectedPath === f.path}
              onToggleStage={onToggleStage}
              onSelect={onSelect}
            />
          ))}
        </Group>
      )}
      {unstaged.length > 0 && (
        <Group
          testId="git-file-list-unstaged"
          label="Unstaged"
          count={unstaged.length}
        >
          {unstaged.map((f) => (
            <Row
              key={f.path}
              file={f}
              checked={false}
              selected={selectedPath === f.path}
              onToggleStage={onToggleStage}
              onSelect={onSelect}
            />
          ))}
        </Group>
      )}
      {untracked.length > 0 && (
        <Group
          testId="git-file-list-untracked"
          label="Untracked"
          count={untracked.length}
        >
          {untracked.map((f) => (
            <Row
              key={f.path}
              file={f}
              checked={false}
              selected={selectedPath === f.path}
              onToggleStage={onToggleStage}
              onSelect={onSelect}
            />
          ))}
        </Group>
      )}
    </Box>
  );
}

function Group({
  testId,
  label,
  count,
  children,
}: {
  testId: string;
  label: string;
  count: number;
  children: React.ReactNode;
}) {
  return (
    <Box data-testid={testId} data-count={count}>
      <Text
        as="div"
        fontFamily="mono"
        fontSize="10px"
        textTransform="uppercase"
        color="fg.subtle"
        px={3}
        py={1}
        bg="bg.muted"
      >
        {label} ({count})
      </Text>
      {children}
    </Box>
  );
}

function Row({
  file,
  checked,
  selected,
  onToggleStage,
  onSelect,
}: {
  file: GitFileChange;
  checked: boolean;
  selected: boolean;
  onToggleStage?: (f: GitFileChange) => void;
  onSelect?: (f: GitFileChange) => void;
}) {
  const status = labelFileStatus(file);
  return (
    <Flex
      data-testid={`git-file-row-${file.path}`}
      data-status={status}
      data-staged={file.staged || undefined}
      data-untracked={file.untracked || undefined}
      data-selected={selected || undefined}
      align="center"
      gap={2}
      px={3}
      py={1}
      bg={selected ? "bg.muted" : undefined}
      _hover={{ bg: "bg.muted" }}
      cursor={onSelect ? "pointer" : undefined}
      onClick={(e) => {
        // Ignore clicks bubbled up from the checkbox so the row click
        // handler does not also flip staging on the same gesture.
        const target = e.target as HTMLElement;
        if (target.closest("[data-stage-checkbox='true']")) return;
        onSelect?.(file);
      }}
    >
      {onToggleStage && (
        <Box
          data-stage-checkbox="true"
          flexShrink={0}
          onClick={(e) => e.stopPropagation()}
        >
          <Checkbox
            data-testid={`git-file-row-${file.path}-stage`}
            checked={checked}
            onCheckedChange={() => onToggleStage(file)}
          />
        </Box>
      )}
      <Text
        as="span"
        fontFamily="mono"
        fontSize="10px"
        color={statusColor(status)}
        flexShrink={0}
        w="14px"
        textAlign="center"
        title={status}
      >
        {statusGlyph(status)}
      </Text>
      <Text
        as="span"
        fontFamily="mono"
        fontSize="11px"
        color="fg"
        flex={1}
        truncate
        title={file.path}
      >
        {file.path}
      </Text>
    </Flex>
  );
}

function statusGlyph(status: string): string {
  switch (status) {
    case "modified":
      return "M";
    case "added":
      return "A";
    case "deleted":
      return "D";
    case "renamed":
      return "R";
    case "copied":
      return "C";
    case "conflicted":
      return "U";
    case "untracked":
      return "?";
    default:
      return "•";
  }
}

function statusColor(status: string): string {
  switch (status) {
    case "modified":
    case "renamed":
    case "copied":
      return "warn";
    case "added":
    case "untracked":
      return "brand.fg";
    case "deleted":
      return "error";
    case "conflicted":
      return "error";
    default:
      return "fg.subtle";
  }
}
