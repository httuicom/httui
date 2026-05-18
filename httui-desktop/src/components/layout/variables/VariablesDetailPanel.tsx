// Canvas §6 Variables — detail panel.
//
// Slice 1 ships the empty-state container only. Slices 2/3/4 layer
// the header / value-per-env / override / used-in-blocks sections
// via the `children` slot — keeping this file size-honest.
// Width comes from `MASTER_DETAIL_DETAIL_WIDTH` so the V5 page lines
// up pixel-for-pixel with V4 Connections.

import { Box, Flex, Text } from "@chakra-ui/react";
import type { ReactNode } from "react";

import { MASTER_DETAIL_DETAIL_WIDTH } from "@/components/layout/shared";

export interface VariablesDetailPanelProps {
  /** Selected variable key. When undefined, the empty state shows. */
  selectedKey?: string | null;
  /** Composition slot for slice 2+ (header, value rows, override, uses). */
  children?: ReactNode;
}

export function VariablesDetailPanel({
  selectedKey,
  children,
}: VariablesDetailPanelProps) {
  return (
    <Flex
      data-testid="variables-detail-panel"
      direction="column"
      w={MASTER_DETAIL_DETAIL_WIDTH}
      minW={MASTER_DETAIL_DETAIL_WIDTH}
      borderLeftWidth="1px"
      borderLeftColor="border"
      bg="bg.subtle"
      h="full"
      overflowY="auto"
    >
      {selectedKey && children ? (
        <Box flex={1} p={4}>
          {children}
        </Box>
      ) : (
        <EmptyState />
      )}
    </Flex>
  );
}

function EmptyState() {
  return (
    <Flex
      data-testid="variables-detail-empty"
      direction="column"
      align="center"
      justify="center"
      h="full"
      px={6}
      gap={2}
    >
      <Text fontFamily="serif" fontSize="16px" color="fg.muted">
        Select a variable
      </Text>
      <Text
        fontSize="11px"
        color="fg.subtle"
        textAlign="center"
        lineHeight={1.5}
      >
        Click a row on the left to view and edit the value per env, configure a
        session override, and list the blocks that use it.
      </Text>
    </Flex>
  );
}
