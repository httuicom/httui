// Epic 53 Story 04 — DbFencedPanel EXPLAIN section.
//
// Presentational composition of the canvas-spec header row +
// `<ExplainPlan>` tree. The consumer (DbFencedPanel) chooses when
// to mount; this component owns only the visual contract.
//
// Hide-entirely contract (Story 04 task 3): when `plan === undefined`
// AND `unsupported` is not set, the section returns `null` so a block
// that hasn't run with `explain=true` shows nothing. `plan === null`
// is the loading state (request in flight); `plan: PlanNode` is the
// ready state; `unsupported` overrides everything with the
// driver-not-supported copy.

import { Box, Flex, Text } from "@chakra-ui/react";

import { ExplainPlan } from "./ExplainPlan";
import type { PlanNode } from "./explain-plan-types";

export interface DbExplainSectionProps {
  /**
   * Root plan node, `null` while a request is in flight, or
   * `undefined` to hide the section entirely (block hasn't been run
   * with `explain=true`).
   */
  plan: PlanNode | null | undefined;
  /**
   * Render the unsupported state regardless of `plan`. Driver name
   * surfaces via `driverLabel`.
   */
  unsupported?: boolean;
  driverLabel?: string;
  /**
   * Optional summary annotation (e.g. `"uses idx_route_provider"`).
   * Renders mono in `fg.ok` to the right of the header.
   */
  summary?: string;
  /**
   * Sub-label below the header. Defaults to `"buffers · timing"`
   * matching the canvas mock.
   */
  subLabel?: string;
}

const DEFAULT_SUB_LABEL = "buffers · timing";

export function DbExplainSection({
  plan,
  unsupported,
  driverLabel,
  summary,
  subLabel = DEFAULT_SUB_LABEL,
}: DbExplainSectionProps) {
  if (plan === undefined && !unsupported) {
    return null;
  }
  return (
    <Box data-testid="db-explain-section" px={3} py={3}>
      <Flex
        align="baseline"
        gap={3}
        mb={2}
        data-testid="db-explain-section-header"
      >
        <Text
          as="span"
          fontSize="10px"
          letterSpacing="0.06em"
          fontWeight={700}
          color="fg"
          flexShrink={0}
          data-testid="db-explain-section-label"
        >
          EXPLAIN ANALYZE
        </Text>
        <Text
          as="span"
          fontFamily="mono"
          fontSize="10px"
          color="fg.subtle"
          flexShrink={0}
          data-testid="db-explain-section-sub"
        >
          {subLabel}
        </Text>
        <Box flex={1} />
        {summary && (
          <Text
            as="span"
            fontFamily="mono"
            fontSize="10px"
            color="fg.ok"
            truncate
            title={summary}
            data-testid="db-explain-section-summary"
          >
            {summary}
          </Text>
        )}
      </Flex>
      <ExplainPlan
        plan={plan ?? null}
        unsupported={unsupported}
        driverLabel={driverLabel}
      />
    </Box>
  );
}
