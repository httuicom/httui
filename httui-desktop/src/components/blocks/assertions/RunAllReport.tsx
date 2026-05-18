// Run-all assertion summary.
//
// Presentational. Consumes the pure `aggregateAssertionResults`
// output. Mounted at the bottom of the run-all stream when the runs
// finish (or stop on first failure when shift-click was NOT held).

import { Box, Flex, Text, chakra } from "@chakra-ui/react";

import type { RunAllAssertionSummary } from "@/lib/blocks/assertions-aggregate";

export interface RunAllReportProps {
  summary: RunAllAssertionSummary;
  /** Click handler invoked when the user clicks a failed block in
   * the report; the consumer scrolls + selects that block. */
  onJumpToBlock?: (blockAlias: string) => void;
}

export function RunAllReport({ summary, onJumpToBlock }: RunAllReportProps) {
  const { blocks, assertions, passed, failed, failedBlocks, allPass } = summary;

  if (assertions === 0) {
    return (
      <Box
        data-testid="run-all-report"
        data-empty="true"
        px={4}
        py={3}
        borderTopWidth="1px"
        borderTopColor="border"
      >
        <Text fontSize="11px" color="fg.subtle">
          {blocks} {blocks === 1 ? "block" : "blocks"} ran. No assertions
          defined.
        </Text>
      </Box>
    );
  }

  return (
    <Box
      data-testid="run-all-report"
      data-pass={allPass || undefined}
      data-fail={!allPass || undefined}
      px={4}
      py={3}
      borderTopWidth="1px"
      borderTopColor="border"
      bg={allPass ? "bg.muted" : "bg.muted"}
    >
      <Flex align="center" gap={2} mb={failedBlocks.length > 0 ? 2 : 0}>
        <Text
          as="span"
          fontSize="14px"
          color={allPass ? "brand.fg" : "error"}
          flexShrink={0}
        >
          {allPass ? "✓" : "✗"}
        </Text>
        <Text
          fontFamily="mono"
          fontSize="11px"
          color="fg"
          data-testid="run-all-report-summary"
        >
          {blocks} {blocks === 1 ? "block" : "blocks"}, {assertions}{" "}
          {assertions === 1 ? "assertion" : "assertions"}, {passed} passed,{" "}
          {failed} failed
        </Text>
      </Flex>

      {failedBlocks.length > 0 && (
        <Flex direction="column" gap={1} pl={5}>
          {failedBlocks.map((alias) => (
            <FailedBlockRow key={alias} alias={alias} onJump={onJumpToBlock} />
          ))}
        </Flex>
      )}
    </Box>
  );
}

function FailedBlockRow({
  alias,
  onJump,
}: {
  alias: string;
  onJump?: (a: string) => void;
}) {
  const interactive = !!onJump;
  const Comp = interactive ? chakra.button : chakra.div;
  return (
    <Comp
      type={interactive ? "button" : undefined}
      data-testid={`run-all-report-failed-block-${alias}`}
      onClick={interactive ? () => onJump?.(alias) : undefined}
      fontFamily="mono"
      fontSize="11px"
      color="error"
      textAlign="left"
      cursor={interactive ? "pointer" : "default"}
      bg="transparent"
      borderWidth={0}
      px={0}
      _hover={interactive ? { textDecoration: "underline" } : undefined}
    >
      ✗ {alias}
    </Comp>
  );
}
