// Epic 51 Story 03 — pill row UI for the DocHeader pre-flight
// checklist.
//
// Pure presentational. Consumer collects `CheckResult[]` from
// `evaluate_preflight` (via Tauri), pairs each with its
// `PreflightItem` for the suggested-action copy, and passes
// `<PreflightPills items={paired} ...>`. Story 04 (Run-all gate)
// reads the same `items` prop independently — pills don't know
// about Run-all.

import { Box, Flex, Text } from "@chakra-ui/react";

import { Btn } from "@/components/atoms";

import {
  pillGlyph,
  pillKindFromResult,
  type CheckResult,
  type PillKind,
} from "./preflight-types";

export interface PreflightPillItem {
  /** Stable id for React keys (use `${idx}-${item_kind}` is fine). */
  id: string;
  /** Short label rendered inside the pill — usually the item name
   *  (`payments-db`, `API_TOKEN`, …). */
  label: string;
  result: CheckResult;
  /** Human suggested action shown on click for failed pills. The
   *  consumer composes this per-kind — "Add this connection",
   *  "Set env var X", etc. */
  suggestion?: string;
}

export interface PreflightPillsProps {
  items: ReadonlyArray<PreflightPillItem>;
  /** True while a Re-check is in flight; flips all pills to the
   *  "running" state regardless of last-known result. */
  rechecking?: boolean;
  onSelectFailure?: (item: PreflightPillItem) => void;
  onRecheck?: () => void;
}

export function PreflightPills({
  items,
  rechecking,
  onSelectFailure,
  onRecheck,
}: PreflightPillsProps) {
  if (items.length === 0) {
    return null;
  }

  return (
    <Flex
      data-testid="preflight-pills"
      data-rechecking={rechecking || undefined}
      data-count={items.length}
      align="center"
      gap={2}
      flexWrap="wrap"
      mt={3}
    >
      {items.map((item) => (
        <Pill
          key={item.id}
          item={item}
          forceRunning={!!rechecking}
          onSelectFailure={onSelectFailure}
        />
      ))}
      {onRecheck && (
        <Btn
          data-testid="preflight-pills-recheck"
          variant="ghost"
          onClick={onRecheck}
          disabled={rechecking}
        >
          {rechecking ? "Re-checking…" : "Re-check"}
        </Btn>
      )}
    </Flex>
  );
}

function Pill({
  item,
  forceRunning,
  onSelectFailure,
}: {
  item: PreflightPillItem;
  forceRunning: boolean;
  onSelectFailure?: (item: PreflightPillItem) => void;
}) {
  const kind: PillKind = forceRunning ? "running" : pillKindFromResult(item.result);
  const isFailWithSuggestion =
    kind === "fail" && !!onSelectFailure;

  const titleParts: string[] = [];
  if (item.result.outcome !== "pass") {
    if ("reason" in item.result) titleParts.push(item.result.reason);
    if (item.suggestion) titleParts.push(item.suggestion);
  }
  const title = titleParts.length > 0 ? titleParts.join(" — ") : undefined;

  return (
    <Box
      as={isFailWithSuggestion ? "button" : "span"}
      data-testid={`preflight-pill-${item.id}`}
      data-kind={kind}
      data-actionable={isFailWithSuggestion || undefined}
      title={title}
      onClick={
        isFailWithSuggestion ? () => onSelectFailure(item) : undefined
      }
      px={2}
      py={1}
      borderRadius="999px"
      bg="bg.muted"
      borderWidth="1px"
      borderColor={pillBorder(kind)}
      cursor={isFailWithSuggestion ? "pointer" : undefined}
      _hover={isFailWithSuggestion ? { bg: "bg.emphasized" } : undefined}
    >
      <Flex align="center" gap={1}>
        <Text
          as="span"
          fontSize="11px"
          color={pillColor(kind)}
          fontWeight={600}
          data-testid={`preflight-pill-${item.id}-glyph`}
        >
          {pillGlyph(kind)}
        </Text>
        <Text
          as="span"
          fontFamily="mono"
          fontSize="11px"
          color="fg.muted"
        >
          {item.label}
        </Text>
      </Flex>
    </Box>
  );
}

function pillBorder(kind: PillKind): string {
  switch (kind) {
    case "pass":
      return "accent";
    case "fail":
      return "error";
    case "running":
      return "warn";
    default:
      return "border";
  }
}

function pillColor(kind: PillKind): string {
  switch (kind) {
    case "pass":
      return "accent";
    case "fail":
      return "error";
    case "running":
      return "warn";
    default:
      return "fg.subtle";
  }
}
