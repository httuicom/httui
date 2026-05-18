// pill row UI for the DocHeader pre-flight
// checklist. added the inline builder (add / edit /
// remove) so users can configure checks visually instead of editing
// the YAML by hand.
//
// Pure presentational for the rendering side; the popover state and
// the callbacks that mutate the frontmatter live in the consumer. The
// pill row owns the popover OPEN/CLOSE state because both the `+ Add
// check` button and click-on-pill are local UI concerns — anchoring
// the popover next to the trigger is a render-time job.

import { useRef, useState } from "react";
import { Box, Flex, Text } from "@chakra-ui/react";
import { LuPlus } from "react-icons/lu";

import { Btn } from "@/components/atoms";

import { PreflightCheckPopover } from "./PreflightCheckPopover";
import { defaultSuggestionProvider } from "./preflight-suggestions";

import type { PreflightCheck } from "@/lib/blocks/preflight-checks";

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
  /** when present, click on the pill opens the
   *  edit popover pre-bound to this kind/value. Without these, the
   *  pill stays as a static read-only chip. */
  kind?: PreflightCheck["kind"];
  value?: string;
}

export interface PreflightPillsProps {
  items: ReadonlyArray<PreflightPillItem>;
  /** True while a Re-check is in flight; flips all pills to the
   *  "running" state regardless of last-known result. */
  rechecking?: boolean;
  onSelectFailure?: (item: PreflightPillItem) => void;
  onRecheck?: () => void;
  /** when wired, surfaces the `+ Add check` button
   *  at the end of the row. `undefined` keeps the read-only legacy
   *  rendering. */
  onAddCheck?: (check: PreflightCheck) => void;
  /** replace the check at index `idx` with `next`. */
  onEditCheck?: (idx: number, next: PreflightCheck) => void;
  /** drop the check at index `idx`. */
  onRemoveCheck?: (idx: number) => void;
}

type PopoverState =
  | { mode: "closed" }
  | { mode: "add"; anchor: DOMRect | null }
  | {
      mode: "edit";
      anchor: DOMRect | null;
      idx: number;
      kind: PreflightCheck["kind"];
      value: string;
    };

export function PreflightPills({
  items,
  rechecking,
  onSelectFailure,
  onRecheck,
  onAddCheck,
  onEditCheck,
  onRemoveCheck,
}: PreflightPillsProps) {
  const [popover, setPopover] = useState<PopoverState>({ mode: "closed" });
  const addBtnRef = useRef<HTMLButtonElement | null>(null);

  const editable = onEditCheck !== undefined && onRemoveCheck !== undefined;

  const close = () => setPopover({ mode: "closed" });

  const openAdd = () => {
    const rect = addBtnRef.current?.getBoundingClientRect() ?? null;
    setPopover({ mode: "add", anchor: rect });
  };

  const openEdit = (
    idx: number,
    kind: PreflightCheck["kind"],
    value: string,
    target: HTMLElement,
  ) => {
    setPopover({
      mode: "edit",
      idx,
      kind,
      value,
      anchor: target.getBoundingClientRect(),
    });
  };

  // Hide entirely when there's nothing to render — keeps the row off
  // the canvas for users who haven't declared any checks yet (the
  // `+ Add check` button moves to the action row in that case;
  // consumers can still mount it when needed).
  if (items.length === 0 && !onAddCheck) {
    return null;
  }

  return (
    <>
      <Flex
        data-testid="preflight-pills"
        data-rechecking={rechecking || undefined}
        data-count={items.length}
        align="center"
        gap={2}
        flexWrap="wrap"
        mt={3}
      >
        {items.map((item, idx) => {
          const canEdit =
            editable && item.kind !== undefined && item.value !== undefined;
          return (
            <Pill
              key={item.id}
              item={item}
              forceRunning={!!rechecking}
              onSelectFailure={onSelectFailure}
              onEdit={
                canEdit
                  ? (target) => openEdit(idx, item.kind!, item.value!, target)
                  : undefined
              }
            />
          );
        })}
        {onAddCheck && (
          <Btn
            ref={addBtnRef}
            data-testid="preflight-pills-add"
            variant="ghost"
            onClick={openAdd}
          >
            <LuPlus size={12} style={{ marginRight: 4 }} />
            Add check
          </Btn>
        )}
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

      {popover.mode === "add" && onAddCheck && (
        <PreflightCheckPopover
          anchorRect={popover.anchor}
          onSave={(check) => {
            onAddCheck(check);
            close();
          }}
          onClose={close}
          getSuggestions={defaultSuggestionProvider}
        />
      )}
      {popover.mode === "edit" && onEditCheck && onRemoveCheck && (
        <PreflightCheckPopover
          anchorRect={popover.anchor}
          initialKind={popover.kind}
          initialValue={popover.value}
          onSave={(check) => {
            onEditCheck(popover.idx, check);
            close();
          }}
          onRemove={() => {
            onRemoveCheck(popover.idx);
            close();
          }}
          onClose={close}
          getSuggestions={defaultSuggestionProvider}
        />
      )}
    </>
  );
}

function Pill({
  item,
  forceRunning,
  onSelectFailure,
  onEdit,
}: {
  item: PreflightPillItem;
  forceRunning: boolean;
  onSelectFailure?: (item: PreflightPillItem) => void;
  /** when wired, click opens the edit popover. Takes
   *  precedence over `onSelectFailure`; the suggestion text remains
   *  in the title attribute as a hover hint. */
  onEdit?: (target: HTMLElement) => void;
}) {
  const kind: PillKind = forceRunning
    ? "running"
    : pillKindFromResult(item.result);
  const editable = !!onEdit;
  const isFailWithSuggestion =
    !editable && kind === "fail" && !!onSelectFailure;
  const interactive = editable || isFailWithSuggestion;

  const titleParts: string[] = [];
  if (item.result.outcome !== "pass") {
    if ("reason" in item.result) titleParts.push(item.result.reason);
    if (item.suggestion) titleParts.push(item.suggestion);
  }
  const title = titleParts.length > 0 ? titleParts.join(" — ") : undefined;

  const handleClick = (e: React.MouseEvent<HTMLElement>) => {
    if (editable && onEdit) {
      onEdit(e.currentTarget);
      return;
    }
    if (isFailWithSuggestion && onSelectFailure) {
      onSelectFailure(item);
    }
  };

  return (
    <Box
      as={interactive ? "button" : "span"}
      data-testid={`preflight-pill-${item.id}`}
      data-kind={kind}
      data-actionable={interactive || undefined}
      data-editable={editable || undefined}
      title={title}
      onClick={interactive ? handleClick : undefined}
      px={2}
      py={1}
      borderRadius="999px"
      bg="bg.muted"
      borderWidth="1px"
      borderColor={pillBorder(kind)}
      cursor={interactive ? "pointer" : undefined}
      _hover={interactive ? { bg: "bg.emphasized" } : undefined}
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
        <Text as="span" fontFamily="mono" fontSize="11px" color="fg.muted">
          {item.label}
        </Text>
      </Flex>
    </Box>
  );
}

function pillBorder(kind: PillKind): string {
  switch (kind) {
    case "pass":
      return "brand.fg";
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
      return "brand.fg";
    case "fail":
      return "error";
    case "running":
      return "warn";
    default:
      return "fg.subtle";
  }
}
