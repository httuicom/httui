// Master-detail sidebar row (V5).
//
// One row inside a sidebar section: icon slot + label + right-aligned
// count. Used by Connections KIND and Variables SCOPES so the rows
// render pixel-identical and stay in sync over time. Pure
// presentational; consumer owns selection state, click handler, and
// the icon node (so kind-coloured icons coexist with plain lucide
// glyphs without bloating the prop API).

import { Box, Text, chakra } from "@chakra-ui/react";
import type { ReactNode } from "react";

const RowButton = chakra("button");

export interface MasterDetailSidebarRowProps {
  /** Leftmost icon slot — caller provides the right element (a
   * `<ConnectionKindIcon>` for kinds, a lucide `<LuX size={18} />`
   * for plain rows). */
  iconSlot: ReactNode;
  label: string;
  /** Right-aligned numeric chip; rendered when defined. */
  count?: number | string;
  selected: boolean;
  onClick: () => void;
  testId?: string;
  /** Optional testid attached to the count chip (consumers that
   * assert per-row counts in tests, e.g. Variables scopes). */
  countTestId?: string;
}

export function MasterDetailSidebarRow({
  iconSlot,
  label,
  count,
  selected,
  onClick,
  testId,
  countTestId,
}: MasterDetailSidebarRowProps) {
  return (
    <RowButton
      type="button"
      data-testid={testId}
      data-selected={selected ? "true" : "false"}
      onClick={onClick}
      display="flex"
      alignItems="center"
      gap={2}
      px={2}
      py="6px"
      borderRadius="6px"
      bg={selected ? "bg.emphasized" : "transparent"}
      cursor="pointer"
      textAlign="left"
      border="none"
      width="100%"
      _hover={{ bg: selected ? "bg.emphasized" : "bg.muted" }}
    >
      <Box
        as="span"
        display="inline-flex"
        alignItems="center"
        justifyContent="center"
        flexShrink={0}
      >
        {iconSlot}
      </Box>
      <Text
        flex={1}
        fontSize="13px"
        fontWeight={selected ? 600 : 500}
        color="fg"
        truncate
      >
        {label}
      </Text>
      {count !== undefined ? (
        <Text
          data-testid={countTestId}
          fontFamily="mono"
          fontSize="11px"
          color="fg.subtle"
          minW="22px"
          textAlign="right"
        >
          {count}
        </Text>
      ) : null}
    </RowButton>
  );
}
