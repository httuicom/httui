// V6 / cenário 10 — Run-all gate handler.
//
// Wires the pure `evaluatePreflightGate` (Epic 51 Story 04) to a
// React state machine + confirmation dialog. Consumers call `trigger`
// (typically from a keyboard shortcut or the action-row Run-all
// button); the hook then either runs immediately (no failures) or
// opens a Portal-based confirmation dialog. The dialog's `Run anyway`
// button replays with `overrideShift: true`.
//
// Pure execution intentionally stays out: the `onRunAll(decision)`
// callback is the seam where the future Run-all flow (Epic 39) hooks
// in. For V6 the callback can no-op or log — the cenário scope is the
// gate, not the actual block execution.

import { useCallback, useState } from "react";
import { Box, Button, Flex, HStack, Portal, Text } from "@chakra-ui/react";

import {
  evaluatePreflightGate,
  type RunAllGateDecision,
} from "@/components/blocks/preflight/run-all-gate";
import type { PreflightPillItem } from "@/components/blocks/preflight/PreflightPills";

export interface UseRunAllGateArgs {
  items: ReadonlyArray<PreflightPillItem>;
  onRunAll: (decision: RunAllGateDecision) => void;
}

export interface UseRunAllGateResult {
  /** Trigger the Run-all flow. Pass `overrideShift: true` to bypass
   *  the gate (e.g. shift-click on the Run-all button). */
  trigger: (overrideShift?: boolean) => void;
  /** JSX to render at the top level — the confirmation dialog when
   *  open, otherwise nothing. Use `{result.dialog}` next to your
   *  editor JSX. */
  dialog: React.ReactNode;
}

export function useRunAllPreflightGate({
  items,
  onRunAll,
}: UseRunAllGateArgs): UseRunAllGateResult {
  const [pending, setPending] = useState<RunAllGateDecision | null>(null);

  const trigger = useCallback(
    (overrideShift?: boolean) => {
      const decision = evaluatePreflightGate({
        results: items.map((i) => i.result),
        overrideShift,
      });
      if (decision.block) {
        setPending(decision);
        return;
      }
      onRunAll(decision);
    },
    [items, onRunAll],
  );

  const cancel = useCallback(() => {
    setPending(null);
  }, []);

  const runAnyway = useCallback(() => {
    if (!pending) return;
    setPending(null);
    // Re-evaluate with the override; the call also produces the
    // audit note carrying "ran anyway via shift" for the future
    // Run-all report.
    const decision = evaluatePreflightGate({
      results: items.map((i) => i.result),
      overrideShift: true,
    });
    onRunAll(decision);
  }, [pending, items, onRunAll]);

  const dialog = pending ? (
    <RunAllConfirm
      decision={pending}
      onCancel={cancel}
      onRunAnyway={runAnyway}
    />
  ) : null;

  return { trigger, dialog };
}

interface RunAllConfirmProps {
  decision: RunAllGateDecision;
  onCancel: () => void;
  onRunAnyway: () => void;
}

function RunAllConfirm({
  decision,
  onCancel,
  onRunAnyway,
}: RunAllConfirmProps) {
  return (
    <Portal>
      <Box
        data-testid="preflight-run-all-confirm-overlay"
        position="fixed"
        inset={0}
        bg="blackAlpha.500"
        zIndex={9998}
        onClick={onCancel}
      />
      <Box
        data-testid="preflight-run-all-confirm"
        role="dialog"
        aria-modal="true"
        position="fixed"
        top="20%"
        left="50%"
        transform="translateX(-50%)"
        w="420px"
        maxW="90vw"
        bg="bg.subtle"
        borderWidth="1px"
        borderColor="border"
        borderRadius="md"
        shadow="2xl"
        zIndex={9999}
        p={5}
      >
        <Text
          fontFamily="mono"
          fontSize="11px"
          color="fg.subtle"
          textTransform="uppercase"
          letterSpacing="0.06em"
          mb={2}
        >
          pre-flight gate
        </Text>
        <Text fontSize="sm" color="fg" mb={4}>
          {decision.confirmCopy}
        </Text>
        {decision.skippedCount > 0 && (
          <Text
            data-testid="preflight-run-all-confirm-skipped"
            fontSize="xs"
            color="fg.muted"
            mb={4}
          >
            {decision.skippedCount} pre-flight check
            {decision.skippedCount === 1 ? "" : "s"} skipped — those won&apos;t
            be re-evaluated by Run anyway.
          </Text>
        )}
        <Flex justify="flex-end">
          <HStack gap={2}>
            <Button
              data-testid="preflight-run-all-confirm-cancel"
              variant="ghost"
              size="sm"
              onClick={onCancel}
            >
              Cancel
            </Button>
            <Button
              data-testid="preflight-run-all-confirm-run-anyway"
              colorPalette="red"
              size="sm"
              onClick={onRunAnyway}
            >
              Run anyway
            </Button>
          </HStack>
        </Flex>
      </Box>
    </Portal>
  );
}
