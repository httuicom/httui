// Epic 53 Story 03 — recursive `PlanNode` tree renderer.
//
// Pure presentational. Consumer (DbFencedPanel — Story 04) fetches
// the parsed plan via Tauri and passes the root node. Each child
// row is indented + connected with vertical / horizontal stub
// lines; a click on a node toggles its subtree. The cost bar runs
// 0..100% with `accent` for normal nodes and `error` when `warn`
// is set.

import { useState } from "react";
import { Box, Flex, Text } from "@chakra-ui/react";

import { type PlanNode, formatRows } from "./explain-plan-types";

export interface ExplainPlanProps {
  /** Root node of the plan tree. `null` while loading or when no
   *  EXPLAIN data is available; the consumer hides the section
   *  in that case. */
  plan: PlanNode | null;
  /** When set, blocks the consumer from showing the plan card —
   *  e.g. the driver doesn't support EXPLAIN. */
  unsupported?: boolean;
  /** Optional driver-name copy for the unsupported message. */
  driverLabel?: string;
}

export function ExplainPlan({
  plan,
  unsupported,
  driverLabel,
}: ExplainPlanProps) {
  if (unsupported) {
    return (
      <Box data-testid="explain-plan" data-state="unsupported" px={3} py={3}>
        <Text fontSize="11px" color="fg.subtle">
          EXPLAIN unavailable for{" "}
          {driverLabel ? <strong>{driverLabel}</strong> : "this driver"}.
        </Text>
      </Box>
    );
  }
  if (plan === null) {
    return (
      <Box data-testid="explain-plan" data-state="loading" px={3} py={3}>
        <Text fontSize="11px" color="fg.subtle">
          Loading plan…
        </Text>
      </Box>
    );
  }
  return (
    <Box data-testid="explain-plan" data-state="ready">
      <Node node={plan} depth={0} isLast />
    </Box>
  );
}

function Node({
  node,
  depth,
  isLast,
}: {
  node: PlanNode;
  depth: number;
  isLast: boolean;
}) {
  const [open, setOpen] = useState(true);
  const hasChildren = node.children.length > 0;
  const nodeId = `${depth}-${node.op}-${node.target}`;
  return (
    <Box
      data-testid={`explain-plan-node`}
      data-op={node.op}
      data-warn={node.warn || undefined}
      data-depth={depth}
      data-last={isLast || undefined}
      data-open={hasChildren ? open : undefined}
      pl={depth === 0 ? 0 : 4}
    >
      <Flex
        align="center"
        gap={2}
        py={1}
        borderLeftWidth={depth > 0 ? "1px" : 0}
        borderLeftColor="border"
        pl={depth > 0 ? 2 : 0}
        cursor={hasChildren ? "pointer" : undefined}
        _hover={hasChildren ? { bg: "bg.muted" } : undefined}
        onClick={hasChildren ? () => setOpen((v) => !v) : undefined}
      >
        {hasChildren && (
          <Text
            as="span"
            fontFamily="mono"
            fontSize="10px"
            color="fg.subtle"
            flexShrink={0}
            w="12px"
            textAlign="center"
            data-testid={`explain-plan-toggle-${nodeId}`}
          >
            {open ? "▾" : "▸"}
          </Text>
        )}
        <Text
          as="span"
          fontFamily="mono"
          fontWeight={600}
          fontSize="11px"
          color={node.warn ? "error" : "fg"}
          flexShrink={0}
          data-testid="explain-plan-op"
        >
          {node.op}
        </Text>
        {node.target && (
          <Text
            as="span"
            fontFamily="mono"
            fontSize="10px"
            color="fg.muted"
            truncate
            flex={1}
            title={node.target}
            data-testid="explain-plan-target"
          >
            {node.target}
          </Text>
        )}
        <Text
          as="span"
          fontFamily="mono"
          fontSize="10px"
          color="fg.subtle"
          flexShrink={0}
          data-testid="explain-plan-cost"
        >
          {node.cost}
        </Text>
        <Text
          as="span"
          fontFamily="mono"
          fontSize="10px"
          color="fg.muted"
          flexShrink={0}
          minW="60px"
          textAlign="right"
          data-testid="explain-plan-rows"
        >
          {formatRows(node.rows)}
        </Text>
        <Box flexShrink={0} w="80px">
          <CostBar pct={node.pct} warn={node.warn} />
        </Box>
        {node.warn && (
          <Text
            as="span"
            fontSize="11px"
            color="error"
            flexShrink={0}
            data-testid="explain-plan-warn-icon"
            title="Heuristic warning"
          >
            ⚠
          </Text>
        )}
      </Flex>
      {hasChildren && open && (
        <Box>
          {node.children.map((child, i) => (
            <Node
              key={i}
              node={child}
              depth={depth + 1}
              isLast={i === node.children.length - 1}
            />
          ))}
        </Box>
      )}
    </Box>
  );
}

function CostBar({ pct, warn }: { pct: number; warn: boolean }) {
  const clamped = Math.max(0, Math.min(100, pct));
  return (
    <Box
      data-testid="explain-plan-cost-bar"
      data-pct={clamped}
      data-warn={warn || undefined}
      h="6px"
      bg="bg.muted"
      borderRadius="3px"
      overflow="hidden"
    >
      <Box h="100%" w={`${clamped}%`} bg={warn ? "error" : "brand.fg"} />
    </Box>
  );
}
