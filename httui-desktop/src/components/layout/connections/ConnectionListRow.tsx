// Canvas §5 — compact connection list row.
// Grid: 26px icon / 1.5fr name+chip / 1.4fr host / 80px env / 70px
// status / 60px uses + ⋮. Selected: 2px accent left border + bg.

import { Box, Flex, HStack, Text, chakra } from "@chakra-ui/react";

import { ConnectionKindIcon } from "./ConnectionKindIcon";
import type { ConnectionKind } from "./connection-kinds";

const RowButton = chakra("button");

export interface ListRowItem {
  /** Stable id (`Connection.id` from the store). */
  id: string;
  name: string;
  /** Optional canvas-spec kind. `null` → fallback icon (sqlite or
   * other unmapped drivers). */
  kind: ConnectionKind | null;
  host: string | null;
  env: string | null;
  /** Latency in ms; `null` while never tested. */
  latencyMs: number | null;
  /** Render dot + latency colour by intent. */
  status: "ok" | "slow" | "down" | "untested";
  /** Hit count from `block_run_history` (canvas: "N uses"). */
  uses: number;
  /** True when the connection name matches the canvas "PROD" chip
   * pattern (case-insensitive substring `prod`). */
  isProd: boolean;
}

export interface ConnectionListRowProps {
  item: ListRowItem;
  selected: boolean;
  onSelect: (id: string) => void;
  /** ⋮ menu trigger. Slice 2 wires no-op handler; slice 3+
   * surfaces Test / Rotate / Duplicate / Delete. */
  onMore?: (id: string) => void;
}

const STATUS_DOT = {
  ok: "green.solid",
  slow: "yellow.solid",
  down: "red.solid",
  untested: "fg.subtle",
} as const;

const STATUS_TEXT = {
  ok: "fg",
  slow: "yellow.fg",
  down: "red.fg",
  untested: "fg.subtle",
} as const;

function fallbackIcon() {
  return (
    <Box
      data-atom="connection-kind-icon"
      data-kind="unknown"
      role="img"
      aria-label="Unknown driver"
      title="Unknown driver"
      h="22px"
      w="22px"
      display="inline-flex"
      alignItems="center"
      justifyContent="center"
      fontSize="14px"
      color="fg.subtle"
      flexShrink={0}
    >
      🗄
    </Box>
  );
}

export function ConnectionListRow({
  item,
  selected,
  onSelect,
  onMore,
}: ConnectionListRowProps) {
  return (
    <RowButton
      type="button"
      data-testid={`connection-row-${item.id}`}
      data-selected={selected ? "true" : "false"}
      onClick={() => onSelect(item.id)}
      display="grid"
      gridTemplateColumns="26px 1.5fr 1.4fr 80px 70px 60px"
      alignItems="center"
      gap="10px"
      px={3}
      py="9px"
      bg={selected ? "accent.soft" : "transparent"}
      borderLeftWidth="2px"
      borderLeftColor={selected ? "accent" : "transparent"}
      borderRadius="4px"
      cursor="pointer"
      textAlign="left"
      border="none"
      _hover={{ bg: selected ? "accent.soft" : "bg.muted" }}
    >
      {item.kind ? <ConnectionKindIcon kind={item.kind} size={22} /> : fallbackIcon()}

      <HStack gap={2} minW={0}>
        <Text
          fontSize="12px"
          fontWeight={selected ? 600 : 500}
          color="fg"
          truncate
        >
          {item.name}
        </Text>
        {item.isProd && (
          <Text
            data-testid={`connection-row-${item.id}-prod`}
            fontSize="9px"
            fontWeight={700}
            letterSpacing="0.08em"
            px="5px"
            py="1px"
            color="red.fg"
            bg="red.subtle"
            borderRadius="3px"
            flexShrink={0}
          >
            PROD
          </Text>
        )}
      </HStack>

      <Text
        fontFamily="mono"
        fontSize="10px"
        color="fg.subtle"
        truncate
      >
        {item.host ?? "—"}
      </Text>

      <Text
        fontFamily="mono"
        fontSize="11px"
        color="fg.muted"
        truncate
      >
        {item.env ?? "—"}
      </Text>

      <Flex align="center" gap={1}>
        <Box
          h="6px"
          w="6px"
          borderRadius="full"
          bg={STATUS_DOT[item.status]}
          flexShrink={0}
          aria-hidden
        />
        <Text
          fontFamily="mono"
          fontSize="11px"
          color={STATUS_TEXT[item.status]}
        >
          {item.latencyMs !== null ? `${item.latencyMs}ms` : "—"}
        </Text>
      </Flex>

      <Flex align="center" justify="flex-end" gap={2}>
        <Text fontFamily="mono" fontSize="11px" color="fg.subtle">
          {item.uses} uses
        </Text>
        <Box
          as="span"
          data-testid={`connection-row-${item.id}-more`}
          onClick={(e: React.MouseEvent) => {
            e.stopPropagation();
            onMore?.(item.id);
          }}
          cursor="pointer"
          color="fg.subtle"
          fontSize="14px"
          px="3px"
          _hover={{ color: "fg.muted" }}
          aria-label="Row actions"
          role="button"
        >
          ⋮
        </Box>
      </Flex>
    </RowButton>
  );
}
