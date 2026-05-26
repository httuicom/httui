// Canvas §5 — Connection-kind glyph + accent color.
//
// Reads metadata from `connection-kinds.ts`. Used by the sidebar
// filter rows and (slice 2) the list-row leftmost column.

import { Box } from "@chakra-ui/react";

import {
  CONNECTION_KINDS,
  kindColor,
  type ConnectionKind,
} from "./connection-kinds";

interface ConnectionKindIconProps {
  kind: ConnectionKind;
  /** Box size in pixels. Canvas spec: 18 in sidebar rows, 22 in
   * list rows, 32 in modal header. */
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
