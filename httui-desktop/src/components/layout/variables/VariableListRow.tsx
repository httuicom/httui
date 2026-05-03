// Canvas §6 Variables — single row (Epic 43 Story 01 slice 2).
//
// Grid mirrors the table headers (1.4fr × 4 + 60px). Col 0 = key with
// scope chip + secret lock, cols 1-3 = first-3-env values (em-dash on
// undefined; ●●●● mask on secret), col 4 = USES count.

import { Box, Flex, Grid, Text } from "@chakra-ui/react";
import { LuLock } from "react-icons/lu";

import type { VariableRow } from "./variable-derive";
import { VARIABLE_SCOPE_META } from "./variable-scopes";

export interface VariableListRowProps {
  row: VariableRow;
  envColumnNames: ReadonlyArray<string>;
  selected?: boolean;
  onClick?: () => void;
}

const SECRET_MASK = "••••••••";

export function VariableListRow({
  row,
  envColumnNames,
  selected,
  onClick,
}: VariableListRowProps) {
  const scopeKey = row.isSecret ? "secret" : row.scope;
  const scopeMeta = VARIABLE_SCOPE_META[scopeKey];
  const visibleEnvs = envColumnNames.slice(0, 3);
  const placeholders = Math.max(0, 3 - visibleEnvs.length);

  return (
    <Grid
      data-testid={`variables-row-${row.key}`}
      data-selected={selected || undefined}
      data-scope={scopeKey}
      role="button"
      tabIndex={0}
      gridTemplateColumns="1.4fr 1.4fr 1.4fr 1.4fr 60px"
      px={5}
      py={2}
      borderBottomWidth="1px"
      borderBottomColor="border"
      bg={selected ? "bg.emphasized" : "transparent"}
      cursor="pointer"
      _hover={{ bg: selected ? "bg.emphasized" : "bg.subtle" }}
      onClick={onClick}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onClick?.();
        }
      }}
      alignItems="center"
    >
      <Flex gap={2} align="center" minW={0}>
        <Box
          as="span"
          aria-hidden
          color="fg.subtle"
          title={scopeMeta.hint}
          data-testid={`variables-row-${row.key}-scope-glyph`}
          display="inline-flex"
        >
          <scopeMeta.icon size={11} />
        </Box>
        <Text
          as="span"
          fontFamily="mono"
          fontSize="12px"
          color="fg"
          truncate
          flex={1}
          data-testid={`variables-row-${row.key}-key`}
        >
          {row.key}
        </Text>
        {row.isSecret && (
          <Box
            as="span"
            aria-label="secret"
            color="fg.subtle"
            data-testid={`variables-row-${row.key}-lock`}
            display="inline-flex"
          >
            <LuLock size={10} />
          </Box>
        )}
      </Flex>

      {visibleEnvs.map((env) => (
        <ValueCell
          key={env}
          value={row.values[env]}
          masked={row.isSecret}
          testId={`variables-row-${row.key}-value-${env}`}
        />
      ))}
      {Array.from({ length: placeholders }).map((_, i) => (
        <Text as="span" key={`ph-${i}`} color="fg.subtle" fontSize="12px">
          —
        </Text>
      ))}

      <Text
        as="span"
        textAlign="right"
        fontFamily="mono"
        fontSize="11px"
        color={row.usesCount > 0 ? "fg.muted" : "fg.subtle"}
        data-testid={`variables-row-${row.key}-uses`}
      >
        {row.usesCount > 0 ? row.usesCount : "—"}
      </Text>
    </Grid>
  );
}

function ValueCell({
  value,
  masked,
  testId,
}: {
  value: string | undefined;
  masked: boolean;
  testId: string;
}) {
  if (value === undefined) {
    return (
      <Text as="span" color="fg.subtle" fontSize="12px" data-testid={testId}>
        —
      </Text>
    );
  }
  if (masked) {
    return (
      <Box
        as="span"
        fontFamily="mono"
        fontSize="11px"
        color="fg.muted"
        data-testid={testId}
      >
        {SECRET_MASK}
      </Box>
    );
  }
  return (
    <Text
      as="span"
      fontFamily="mono"
      fontSize="11px"
      color="fg"
      truncate
      title={value}
      data-testid={testId}
    >
      {value}
    </Text>
  );
}
