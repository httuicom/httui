import { Box, Flex, Text } from "@chakra-ui/react";
import { LuLock } from "react-icons/lu";

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
        <Box
          as="span"
          aria-hidden
          color="fg.muted"
          data-testid="variable-detail-header-glyph"
          display="inline-flex"
        >
          <meta.icon size={14} />
        </Box>
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
          <Flex
            as="span"
            data-testid="variable-detail-header-secret-chip"
            align="center"
            gap={1}
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
            <LuLock size={10} />
            secret
          </Flex>
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
