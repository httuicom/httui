import { useCallback } from "react";
import { Box, Flex, HStack, Text, VStack } from "@chakra-ui/react";
import {
  DndContext,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import { LuArchive, LuArchiveRestore } from "react-icons/lu";

import { useWorkspace } from "@/contexts/WorkspaceContext";
import { useArchiveFilterStore } from "@/stores/archiveFilter";
import { useTagIndexStore } from "@/stores/tagIndex";
import { FileTreeNode } from "./FileTreeNode";
import { InlineInput } from "./InlineInput";
import { resolveFileTreeDrop } from "./file-tree-drag";

export function FileTree() {
  const {
    entries,
    inlineCreate,
    handleCreateNote,
    handleCreateFolder,
    handleMoveFile,
    cancelInlineCreate,
  } = useWorkspace();

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 5 } }),
  );

  const handleDragEnd = useCallback(
    (event: DragEndEvent) => {
      const drop = resolveFileTreeDrop(event);
      if (!drop) return;
      handleMoveFile(drop.sourcePath, drop.targetDir);
    },
    [handleMoveFile],
  );

  const showRootInline = inlineCreate && inlineCreate.dirPath === "";

  if (entries.length === 0 && !showRootInline) {
    return (
      <Box px={3} py={8} textAlign="center">
        <Text fontSize="sm" color="fg.muted">
          Empty vault
        </Text>
      </Box>
    );
  }

  return (
    <DndContext sensors={sensors} onDragEnd={handleDragEnd}>
      <VStack align="stretch" gap={0} px={1}>
        <ArchiveFilterToggle />
        {showRootInline && (
          <InlineInput
            type={inlineCreate.type}
            depth={0}
            onConfirm={(name) => {
              if (inlineCreate.type === "note") handleCreateNote("", name);
              else handleCreateFolder("", name);
            }}
            onCancel={cancelInlineCreate}
          />
        )}
        {entries.map((entry) => (
          <FileTreeNode key={entry.path} entry={entry} depth={0} />
        ))}
      </VStack>
    </DndContext>
  );
}

/** V6 / cenário 8 — small toggle that flips `useArchiveFilterStore.
 *  showArchived`. Hidden entirely when the vault has no archived
 *  files yet so the chrome stays clean. */
function ArchiveFilterToggle() {
  const archivedCount = useTagIndexStore(
    (s) => Object.keys(s.archivedFiles).length,
  );
  const showArchived = useArchiveFilterStore((s) => s.showArchived);
  const toggleShowArchived = useArchiveFilterStore((s) => s.toggleShowArchived);
  if (archivedCount === 0) return null;
  return (
    <Flex
      data-testid="file-tree-archive-toggle"
      data-show-archived={showArchived || undefined}
      as="button"
      align="center"
      justify="space-between"
      px={2}
      py={1}
      mb={1}
      rounded="md"
      bg="transparent"
      color={showArchived ? "fg" : "fg.subtle"}
      _hover={{ bg: "bg.subtle" }}
      onClick={toggleShowArchived}
      title={
        showArchived
          ? "Hide archived notes"
          : `Show archived notes (${archivedCount})`
      }
    >
      <HStack gap={1.5}>
        <Box flexShrink={0}>
          {showArchived ? (
            <LuArchiveRestore size={12} />
          ) : (
            <LuArchive size={12} />
          )}
        </Box>
        <Text fontSize="xs" fontFamily="mono" letterSpacing="0.02em">
          {showArchived ? "Hide archived" : `Show archived (${archivedCount})`}
        </Text>
      </HStack>
    </Flex>
  );
}
