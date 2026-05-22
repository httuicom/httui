import { Box, Flex, Text } from "@chakra-ui/react";

import type { OutlineEntry } from "@/lib/blocks/outline";

export interface OutlineListProps {
  entries: OutlineEntry[];
  /** 1-indexed cursor line; highlights the heading whose `line` is the
   *  largest value `<= activeLine`. */
  activeLine?: number;
  /** Positional `1. 2. …` numbering. Defaults to true. */
  numbered?: boolean;
  onSelect?: (entry: OutlineEntry) => void;
}

export function OutlineList({
  entries,
  activeLine,
  numbered = true,
  onSelect,
}: OutlineListProps) {
  if (entries.length === 0) {
    return (
      <Box
        data-testid="outline-empty"
        px={3}
        py={2}
        fontFamily="mono"
        fontSize="11px"
        color="fg.subtle"
      >
        No headings yet
      </Box>
    );
  }
  const activeIndex =
    activeLine === undefined ? -1 : pickActiveIndex(entries, activeLine);

  return (
    <Box data-testid="outline-list" role="navigation" aria-label="Outline">
      {entries.map((entry, idx) => {
        const indent = (entry.level - 1) * 12;
        const active = idx === activeIndex;
        const interactive = !!onSelect;
        return (
          <Flex
            key={`${entry.line}-${idx}`}
            as={interactive ? "button" : "div"}
            data-testid="outline-row"
            data-level={entry.level}
            data-line={entry.line}
            data-active={active ? "true" : "false"}
            align="baseline"
            gap={2}
            px={3}
            py="3px"
            pl={`${12 + indent}px`}
            width="100%"
            textAlign="left"
            bg={active ? "bg.muted" : "transparent"}
            color={active ? "fg.0" : "fg.1"}
            cursor={interactive ? "pointer" : undefined}
            _hover={interactive ? { bg: "bg.muted" } : undefined}
            onClick={interactive ? () => onSelect(entry) : undefined}
          >
            {numbered && (
              <Text
                fontFamily="mono"
                fontSize="10px"
                color={active ? "brand.fg" : "fg.subtle"}
                minWidth="22px"
              >
                {idx + 1}.
              </Text>
            )}
            <Text
              fontSize={entry.level === 1 ? "13px" : "12px"}
              fontWeight={entry.level === 1 ? 600 : 400}
              lineClamp={1}
              flex={1}
              title={entry.text}
            >
              {entry.text}
            </Text>
          </Flex>
        );
      })}
    </Box>
  );
}

/** Pick the entry whose `line` is the largest value `<= activeLine`.
 *  Returns -1 when `activeLine` is before every heading (cursor in
 *  the preamble) so nothing highlights. */
function pickActiveIndex(entries: OutlineEntry[], activeLine: number): number {
  let best = -1;
  for (let i = 0; i < entries.length; i += 1) {
    if (entries[i].line <= activeLine) {
      best = i;
    } else {
      break;
    }
  }
  return best;
}
