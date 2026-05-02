// Canvas §6 Variables — detail panel header (Epic 43 Story 02 slice 1).
//
// Renders the selected variable's identity strip at the top of the
// 380px detail panel: scope glyph + serif key + scope label, with a
// 🔒 lock chip when the var is a secret. Pure presentational.

import { Box, Flex, Text } from "@chakra-ui/react";

import type { VariableRow } from "./variable-derive";
import { VARIABLE_SCOPE_META } from "./variable-scopes";

export interface VariableDetailHeaderProps {
  row: VariableRow;
}

export function VariableDetailHeader({ row }: VariableDetailHeaderProps) {
  const scopeKey = row.isSecret ? "secret" : row.scope;
  const meta = VARIABLE_SCOPE_META[scopeKey];

  return (
    <Box
      data-testid="variable-detail-header"
      data-scope={scopeKey}
      borderBottomWidth="1px"
      borderBottomColor="border"
      px={4}
      py={3}
    >
      <Flex align="center" gap={2}>
        <Text
          as="span"
          aria-hidden
          fontSize="14px"
          color="fg.muted"
          data-testid="variable-detail-header-glyph"
        >
          {meta.glyph}
        </Text>
        <Text
          as="span"
          fontFamily="mono"
          fontSize="14px"
          color="fg"
          truncate
          flex={1}
          data-testid="variable-detail-header-key"
        >
          {row.key}
        </Text>
        {row.isSecret && (
          <Box
            as="span"
            data-testid="variable-detail-header-secret-chip"
            fontFamily="mono"
            fontSize="10px"
            color="fg.muted"
            bg="bg"
            borderWidth="1px"
            borderColor="border"
            borderRadius="999px"
            px={2}
            py={0.5}
          >
            🔒 secret
          </Box>
        )}
      </Flex>
      <Text
        fontSize="11px"
        color="fg.subtle"
        mt={1}
        data-testid="variable-detail-header-hint"
      >
        {meta.label} — {meta.hint}
      </Text>
    </Box>
  );
}
