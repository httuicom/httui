// Connection-kind glyph + accent color, sourced from `connection-kinds.ts`.

import { Box } from "@chakra-ui/react";

import {
  CONNECTION_KINDS,
  kindColor,
  type ConnectionKind,
} from "./connection-kinds";

interface ConnectionKindIconProps {
  kind: ConnectionKind;
  /** Box size in pixels (18 sidebar, 22 list row, 32 modal header). */
  size?: number;
}

export function ConnectionKindIcon({
  kind,
  size = 18,
}: ConnectionKindIconProps) {
  const meta = CONNECTION_KINDS[kind];
  const Icon = meta.Icon;
  return (
    <Box
      data-atom="connection-kind-icon"
      data-kind={kind}
      aria-label={meta.label}
      title={meta.label}
      role="img"
      display="inline-flex"
      alignItems="center"
      justifyContent="center"
      h={`${size}px`}
      w={`${size}px`}
      lineHeight={1}
      color={kindColor(kind)}
      flexShrink={0}
    >
      <Icon size={Math.round(size * 0.78)} />
    </Box>
  );
}
