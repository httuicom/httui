import { Box } from "@chakra-ui/react";

export type DocHeaderStatusValue = "draft" | "active" | "archived";

export interface DocHeaderStatusBadgeProps {
  /** `frontmatter.status` from the parser. Hidden when
   *  null / undefined / empty so consumer mounts unconditionally. */
  status?: string | null;
  /** Optional click handler; turns the badge into a `<button>`
   *  with hover tone and exposes it as an interactive element so
   *  consumers can route to a status filter or settings panel. */
  onSelect?: (status: string) => void;
}

interface BadgePalette {
  bg: string;
  color: string;
  borderColor: string;
}

const PALETTES: Record<DocHeaderStatusValue, BadgePalette> = {
  draft: { bg: "bg.muted", color: "fg.muted", borderColor: "border.2" },
  active: { bg: "bg.muted", color: "brand.fg", borderColor: "brand.fg" },
  archived: { bg: "bg.muted", color: "fg.subtle", borderColor: "border.2" },
};

const FALLBACK_PALETTE: BadgePalette = {
  bg: "bg.muted",
  color: "fg.subtle",
  borderColor: "border.2",
};

const KNOWN_STATUSES = new Set<DocHeaderStatusValue>([
  "draft",
  "active",
  "archived",
]);

export function DocHeaderStatusBadge({
  status,
  onSelect,
}: DocHeaderStatusBadgeProps) {
  if (!status || !status.trim()) return null;
  const normalized = status.trim().toLowerCase();
  const known = KNOWN_STATUSES.has(normalized as DocHeaderStatusValue);
  const palette = known
    ? PALETTES[normalized as DocHeaderStatusValue]
    : FALLBACK_PALETTE;
  const interactive = !!onSelect;

  const handleClick = interactive ? () => onSelect?.(normalized) : undefined;

  return (
    <Box
      as={interactive ? "button" : "span"}
      data-testid="docheader-status-badge"
      data-status={normalized}
      data-known={known ? "true" : "false"}
      display="inline-flex"
      alignItems="center"
      px={2}
      py="1px"
      fontFamily="mono"
      fontSize="10px"
      letterSpacing="0.06em"
      fontWeight={700}
      textTransform="uppercase"
      borderRadius="3px"
      borderWidth="1px"
      borderStyle="solid"
      bg={palette.bg}
      color={palette.color}
      borderColor={palette.borderColor}
      cursor={interactive ? "pointer" : undefined}
      _hover={interactive ? { bg: "bg.emphasized" } : undefined}
      onClick={handleClick}
      title={`status: ${normalized}`}
    >
      {normalized}
    </Box>
  );
}
