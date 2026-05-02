// Epic 50 Story 05 — action buttons row at the top-right of the
// DocHeader card. ▶ Run all (mirrors the top-bar button but
// scoped to this file), ↗ Share (opens the Epic 49 popover),
// … overflow (Duplicate / Archive / Delete).
//
// Pure presentational. The consumer wires `onRunAll`, `onShare`,
// and the per-action overflow callbacks; we don't open any
// popover ourselves to keep this row test-friendly and to avoid
// pulling Chakra Menu primitives into the card surface (the popovers
// are owned by the consumer's panel — Epic 49 + the file ops menu).

import { useState } from "react";
import { Box, Flex, Text } from "@chakra-ui/react";

import { Btn } from "@/components/atoms";

export interface DocHeaderActionRowProps {
  onRunAll?: () => void;
  /** Disables the Run-all button — used while a run is in flight. */
  runAllBusy?: boolean;
  onShare?: () => void;
  onDuplicate?: () => void;
  onArchive?: () => void;
  onDelete?: () => void;
}

export function DocHeaderActionRow({
  onRunAll,
  runAllBusy,
  onShare,
  onDuplicate,
  onArchive,
  onDelete,
}: DocHeaderActionRowProps) {
  const [overflowOpen, setOverflowOpen] = useState(false);
  const hasOverflow = !!(onDuplicate || onArchive || onDelete);

  return (
    <Flex
      data-testid="docheader-action-row"
      gap={2}
      align="center"
      flexShrink={0}
      position="relative"
    >
      {onRunAll && (
        <Btn
          data-testid="docheader-action-run-all"
          data-busy={runAllBusy || undefined}
          variant="primary"
          disabled={runAllBusy}
          onClick={onRunAll}
        >
          ▶ Run all
        </Btn>
      )}
      {onShare && (
        <Btn
          data-testid="docheader-action-share"
          variant="ghost"
          onClick={onShare}
        >
          ↗ Share
        </Btn>
      )}
      {hasOverflow && (
        <Box position="relative">
          <Btn
            data-testid="docheader-action-overflow"
            data-open={overflowOpen || undefined}
            variant="ghost"
            onClick={() => setOverflowOpen((v) => !v)}
            aria-expanded={overflowOpen}
            aria-haspopup="menu"
          >
            …
          </Btn>
          {overflowOpen && (
            <OverflowMenu
              onClose={() => setOverflowOpen(false)}
              onDuplicate={onDuplicate}
              onArchive={onArchive}
              onDelete={onDelete}
            />
          )}
        </Box>
      )}
    </Flex>
  );
}

function OverflowMenu({
  onClose,
  onDuplicate,
  onArchive,
  onDelete,
}: {
  onClose: () => void;
  onDuplicate?: () => void;
  onArchive?: () => void;
  onDelete?: () => void;
}) {
  return (
    <Box
      data-testid="docheader-action-overflow-menu"
      role="menu"
      position="absolute"
      top="calc(100% + 4px)"
      right={0}
      bg="bg.subtle"
      borderWidth="1px"
      borderColor="border"
      borderRadius="4px"
      minW="160px"
      py={1}
      zIndex={10}
    >
      {onDuplicate && (
        <MenuItem
          testId="docheader-action-duplicate"
          onClick={() => {
            onDuplicate();
            onClose();
          }}
        >
          Duplicate
        </MenuItem>
      )}
      {onArchive && (
        <MenuItem
          testId="docheader-action-archive"
          onClick={() => {
            onArchive();
            onClose();
          }}
        >
          Archive
        </MenuItem>
      )}
      {onDelete && (
        <MenuItem
          testId="docheader-action-delete"
          tone="error"
          onClick={() => {
            onDelete();
            onClose();
          }}
        >
          Delete…
        </MenuItem>
      )}
    </Box>
  );
}

function MenuItem({
  testId,
  tone,
  onClick,
  children,
}: {
  testId: string;
  tone?: "error";
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <Text
      as="button"
      role="menuitem"
      data-testid={testId}
      data-tone={tone}
      display="block"
      w="100%"
      textAlign="left"
      px={3}
      py={1}
      fontFamily="mono"
      fontSize="11px"
      color={tone === "error" ? "error" : "fg"}
      onClick={onClick}
      _hover={{ bg: "bg.muted" }}
      cursor="pointer"
    >
      {children}
    </Text>
  );
}
