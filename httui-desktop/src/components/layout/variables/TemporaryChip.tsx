// Canvas §6 Variables — TEMPORARY chip (Epic 43 Story 03).
//
// Tiny accent-bg chip with serif italic label, marking a variable
// value that is currently being overridden in the session. When
// `onClear` is supplied the chip becomes a button that drops the
// override on click.

import { Box } from "@chakra-ui/react";

export interface TemporaryChipProps {
  /** Click handler to clear the override. When omitted the chip is purely informational. */
  onClear?: () => void;
  /** Optional label override (default: "TEMPORARY"). */
  label?: string;
}

export function TemporaryChip({
  onClear,
  label = "TEMPORARY",
}: TemporaryChipProps) {
  const interactive = !!onClear;
  return (
    <Box
      as={interactive ? "button" : "span"}
      type={interactive ? "button" : undefined}
      data-testid="temporary-chip"
      data-interactive={interactive || undefined}
      onClick={onClear}
      bg="brand.fg"
      color="brand.contrast"
      fontFamily="serif"
      fontStyle="italic"
      fontSize="9px"
      letterSpacing="0.04em"
      px={1.5}
      py={0.5}
      borderRadius="4px"
      cursor={interactive ? "pointer" : "default"}
      borderWidth={0}
      title={interactive ? "Clear session override" : undefined}
      _hover={interactive ? { opacity: 0.85 } : undefined}
    >
      {label}
    </Box>
  );
}
