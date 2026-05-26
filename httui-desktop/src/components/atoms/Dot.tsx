// Status indicator atom — 6×6 circle, `STATE_COLORS` palette plus an
// `idle` variant (design canvas §0).

import { Box, type BoxProps } from "@chakra-ui/react";

import { STATE_COLORS } from "@/theme/tokens";

export type DotVariant = "ok" | "warn" | "err" | "info" | "idle";

export type DotProps = Omit<BoxProps, "as"> & {
  variant?: DotVariant;
};

const VARIANT_BG: Record<DotVariant, string> = {
  ok: STATE_COLORS.ok,
  warn: STATE_COLORS.warn,
  err: STATE_COLORS.err,
  info: STATE_COLORS.info,
  // `idle` is the absence of state — use the muted-fg ramp so the dot
  // reads as "off" rather than "ok green".
  idle: "var(--chakra-colors-fg-3)",
};

export function Dot({ variant = "idle", ...rest }: DotProps) {
  return (
    <Box
      data-atom="dot"
      data-variant={variant}
      width="6px"
      height="6px"
      borderRadius="full"
      bg={VARIANT_BG[variant]}
      flexShrink={0}
      aria-hidden
      {...rest}
    />
  );
}
