// Canvas §5 — Detail panel "used in runbooks" section
// (Epic 42 Story 04).
//
// Shows a click-to-open list of file:line entries where the
// selected connection's `db-<id>` fenced block appears. Pure
// presentational — consumer drives the list (typically derived
// via `findUsagesAcrossVault` from `connection-usages.ts` against
// a vault grep).

import { Box, Flex, Stack, Text, chakra } from "@chakra-ui/react";

import type { RunbookUsage } from "./connection-usages";

const UsageButton = chakra("button");

export interface ConnectionDetailUsedInProps {
  usages: RunbookUsage[];
  /** True while the consumer is loading the vault grep. */
  loading?: boolean;
  /** Click on a row → open the file at that line. Consumer
   * routes through `useEditorSession.handleFileSelect` + a
   * cursor scroll. */
  onOpen?: (filePath: string, line: number) => void;
}

export function ConnectionDetailUsedIn({
  usages,
  loading = false,
  onOpen,
}: ConnectionDetailUsedInProps) {
  return (
    <Stack data-testid="connection-used-in" gap={2} align="stretch">
      <Flex justify="space-between" align="center">
        <Text
          fontFamily="mono"
          fontSize="11px"
          fontWeight="bold"
          letterSpacing="0.08em"
          textTransform="uppercase"
          color="fg.muted"
        >
          Used in runbooks
        </Text>
        <Text
          fontFamily="mono"
          fontSize="11px"
          color="fg.subtle"
          data-testid="used-in-count"
        >
          {usages.length}
        </Text>
      </Flex>

      {loading && usages.length === 0 && (
        <Text data-testid="used-in-loading" fontSize="11px" color="fg.subtle">
          Searching vault…
        </Text>
      )}

      {!loading && usages.length === 0 && (
        <Text data-testid="used-in-empty" fontSize="11px" color="fg.subtle">
          Not referenced in any runbook yet.
        </Text>
      )}

      {usages.length > 0 && (
        <Stack gap={0.5} align="stretch" data-testid="used-in-list">
          {usages.map((u, i) => (
            <UsageButton
              key={`${u.filePath}:${u.line}:${i}`}
              type="button"
              data-testid={`used-in-row-${i}`}
              onClick={() => onOpen?.(u.filePath, u.line)}
              display="block"
              w="full"
              textAlign="left"
              bg="transparent"
              border="none"
              cursor="pointer"
              borderRadius="4px"
              px={2}
              py="6px"
              _hover={{ bg: "bg.muted" }}
            >
              <Flex align="baseline" gap={2}>
                <Text
                  fontFamily="mono"
                  fontSize="11px"
                  color="fg"
                  flex={1}
                  truncate
                >
                  {u.filePath}
                </Text>
                <Text
                  fontFamily="mono"
                  fontSize="10px"
                  color="fg.subtle"
                  flexShrink={0}
                >
                  :{u.line}
                </Text>
              </Flex>
              {u.preview !== null && (
                <Text
                  data-testid={`used-in-row-${i}-preview`}
                  fontFamily="mono"
                  fontSize="10px"
                  color="fg.subtle"
                  truncate
                  mt={0.5}
                >
                  {u.preview}
                </Text>
              )}
            </UsageButton>
          ))}
        </Stack>
      )}
      <Box />
    </Stack>
  );
}
