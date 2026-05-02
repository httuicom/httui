// Canvas §6 Variables — 380px detail panel (Epic 43 Story 01 slice 1).
//
// Slice 1 ships the empty-state container only. Slices 2/3/4 layer
// the header / valor-por-ambiente / override / used-in-blocks
// sections via the `children` slot — keeping this file size-honest.

import { Box, Flex, Text } from "@chakra-ui/react";
import type { ReactNode } from "react";

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
      w="380px"
      minW="380px"
      borderLeftWidth="1px"
      borderLeftColor="border"
      bg="bg.muted"
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
        Selecione uma variável
      </Text>
      <Text fontSize="11px" color="fg.subtle" textAlign="center" lineHeight={1.5}>
        Clique em uma linha à esquerda para ver e editar o valor por ambiente,
        configurar override de sessão e listar os blocos que a usam.
      </Text>
    </Flex>
  );
}
