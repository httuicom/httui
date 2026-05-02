// Block-status assertions badge — Epic 45 Story 04.
//
// Compact `N/M` chip mounted next to the run badge in the panel
// status bar. Color shifts on failure. Renders nothing when there
// are no assertions to evaluate (the consumer can short-circuit on
// `total === 0` without an outer guard).

import { Box } from "@chakra-ui/react";

import type { AssertionResult } from "@/lib/blocks/assertions";

export interface AssertionsBadgeProps {
  /** Total number of parsed assertions in the block. 0 hides the badge. */
  total: number;
  /** Aggregate result from `useAssertionResult`. Null = block hasn't
   * run yet → badge shows the spec'd-but-pending state. */
  result: AssertionResult | null;
  onClick?: () => void;
}

export function AssertionsBadge({
  total,
  result,
  onClick,
}: AssertionsBadgeProps) {
  if (total === 0) return null;

  const passed = result ? total - result.failures.length : null;
  const allPass = result?.pass === true;
  const someFail = result?.pass === false;

  const bg = someFail ? "error" : allPass ? "brand.fg" : "bg";
  const fg = someFail || allPass ? "brand.contrast" : "fg.muted";
  const border = someFail || allPass ? "transparent" : "border";

  const label = result === null ? `0/${total}` : `${passed}/${total}`;

  const interactive = !!onClick;

  return (
    <Box
      as={interactive ? "button" : "span"}
      type={interactive ? "button" : undefined}
      data-testid="assertions-badge"
      data-pass={allPass || undefined}
      data-fail={someFail || undefined}
      data-pending={result === null || undefined}
      onClick={onClick}
      bg={bg}
      color={fg}
      borderWidth="1px"
      borderColor={border}
      borderRadius="999px"
      fontFamily="mono"
      fontSize="10px"
      px={2}
      py={0.5}
      cursor={interactive ? "pointer" : "default"}
      title={
        someFail
          ? `${result.failures.length} assertion${result.failures.length === 1 ? "" : "s"} failed`
          : allPass
            ? "all assertions passed"
            : "assertions awaiting first run"
      }
    >
      ✓ {label}
    </Box>
  );
}
