
import { Box, Flex, Text, chakra } from "@chakra-ui/react";

import type { VarUseEntry } from "@/lib/tauri/var-uses";

import { groupVarUsesByFile } from "./used-in-blocks-group";

export interface UsedInBlocksListProps {
  entries: ReadonlyArray<VarUseEntry> | undefined;
  loading?: boolean;
  error?: string | null;
  onJump?: (filePath: string, line: number) => void;
}

export function UsedInBlocksList({
  entries,
  loading,
  error,
  onJump,
}: UsedInBlocksListProps) {
  if (loading) {
    return (
      <Text
        data-testid="used-in-blocks-loading"
        fontSize="11px"
        color="fg.subtle"
        px={4}
        py={2}
      >
        Searching references…
      </Text>
    );
  }
  if (error) {
    return (
      <Text
        data-testid="used-in-blocks-error"
        fontSize="11px"
        color="error"
        px={4}
        py={2}
      >
        ⚠ {error}
      </Text>
    );
  }
  if (!entries || entries.length === 0) {
    return (
      <Text
        data-testid="used-in-blocks-empty"
        fontSize="11px"
        color="fg.subtle"
        px={4}
        py={2}
      >
        No references found in the vault.
      </Text>
    );
  }

  const groups = groupVarUsesByFile(entries);

  return (
    <Flex
      direction="column"
      data-testid="used-in-blocks-list"
      data-count={entries.length}
    >
      {groups.map((group) => (
        <Box
          key={group.filePath}
          data-testid={`used-in-blocks-group-${group.filePath}`}
        >
          <Text
            fontFamily="mono"
            fontSize="10px"
            fontWeight="bold"
            color="fg.muted"
            px={4}
            pt={2}
            pb={1}
            data-testid={`used-in-blocks-file-${group.filePath}`}
          >
            {group.filePath}{" "}
            <Text as="span" color="fg.subtle" fontWeight="normal">
              ({group.hits.length})
            </Text>
          </Text>
          {group.hits.map((hit) => (
            <Hit
              key={`${group.filePath}:${hit.line}`}
              filePath={group.filePath}
              line={hit.line}
              snippet={hit.snippet}
              onJump={onJump}
            />
          ))}
        </Box>
      ))}
    </Flex>
  );
}

function Hit({
  filePath,
  line,
  snippet,
  onJump,
}: {
  filePath: string;
  line: number;
  snippet: string;
  onJump?: (filePath: string, line: number) => void;
}) {
  const interactive = !!onJump;
  const Comp = interactive ? chakra.button : chakra.div;
  return (
    <Comp
      type={interactive ? "button" : undefined}
      data-testid={`used-in-blocks-hit-${filePath}:${line}`}
      onClick={interactive ? () => onJump?.(filePath, line) : undefined}
      display="flex"
      alignItems="baseline"
      gap={2}
      px={4}
      py={1}
      cursor={interactive ? "pointer" : "default"}
      borderWidth={0}
      bg="transparent"
      textAlign="left"
      w="full"
      _hover={interactive ? { bg: "bg.subtle" } : undefined}
    >
      <Text
        as="span"
        fontFamily="mono"
        fontSize="10px"
        color="fg.subtle"
        flexShrink={0}
        w="32px"
        textAlign="right"
      >
        {line}
      </Text>
      <Text
        as="span"
        fontFamily="mono"
        fontSize="11px"
        color="fg"
        truncate
        title={snippet}
      >
        {snippet}
      </Text>
    </Comp>
  );
}
