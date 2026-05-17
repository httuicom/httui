// Right-sidebar History tab list (Epic 29 Story 01).
//
// Pure presentational. Consumer fetches `block_run_history` rows
// via existing `list_block_history` Tauri cmd, optionally filters
// by current-runbook, and feeds the array. Click → fires
// `onSelect(entry)` (Story 02 task 1: navigate to block + open
// past run); right-click context menu carries (Story 02 task 2).

import { Box, Flex, Text } from "@chakra-ui/react";

import type { HistoryEntry } from "@/lib/tauri/commands";

export type HistoryOutcomeTone = "ok" | "warn" | "err" | "muted";

export interface HistoryListProps {
  entries: HistoryEntry[];
  /** When supplied, rows become buttons firing `onSelect(entry)`. */
  onSelect?: (entry: HistoryEntry) => void;
}

export function HistoryList({ entries, onSelect }: HistoryListProps) {
  if (entries.length === 0) {
    return (
      <Box
        data-testid="history-empty"
        px={3}
        py={2}
        fontFamily="mono"
        fontSize="11px"
        color="fg.subtle"
      >
        No runs yet
      </Box>
    );
  }
  return (
    <Box data-testid="history-list" role="list">
      {entries.map((entry) => {
        const tone = outcomeTone(entry);
        const interactive = !!onSelect;
        return (
          <Flex
            key={entry.id}
            as={interactive ? "button" : "div"}
            data-testid="history-row"
            data-id={entry.id}
            data-tone={tone}
            role={interactive ? undefined : "listitem"}
            align="baseline"
            gap={2}
            px={3}
            py="4px"
            width="100%"
            textAlign="left"
            cursor={interactive ? "pointer" : undefined}
            _hover={interactive ? { bg: "bg.muted" } : undefined}
            onClick={interactive ? () => onSelect(entry) : undefined}
          >
            <Box
              data-testid="history-row-dot"
              width="6px"
              height="6px"
              borderRadius="50%"
              bg={dotColor(tone)}
              flexShrink={0}
            />
            <Text
              fontFamily="mono"
              fontSize="10px"
              color="fg.subtle"
              minWidth="36px"
            >
              {formatRelative(entry.ran_at)}
            </Text>
            <Text
              fontFamily="mono"
              fontSize="11px"
              color="fg.1"
              lineClamp={1}
              flex={1}
              title={`${entry.method} ${entry.url_canonical}`}
            >
              {label(entry)}
            </Text>
            {hasPlan(entry) && (
              <Box
                data-testid="history-row-plan"
                fontFamily="mono"
                fontSize="10px"
                color="fg.subtle"
                px="4px"
                py="1px"
                borderRadius="3px"
                bg="bg.muted"
                flexShrink={0}
                title="EXPLAIN plan captured for this run"
              >
                📊 plan
              </Box>
            )}
            {entry.status !== null && (
              <Text
                fontFamily="mono"
                fontSize="10px"
                color={dotColor(tone)}
                minWidth="28px"
                textAlign="right"
              >
                {entry.status}
              </Text>
            )}
            {entry.elapsed_ms !== null && (
              <Text
                fontFamily="mono"
                fontSize="10px"
                color="fg.subtle"
                minWidth="36px"
                textAlign="right"
              >
                {formatElapsed(entry.elapsed_ms)}
              </Text>
            )}
          </Flex>
        );
      })}
    </Box>
  );
}

/** Pure pick of the visual tone for a run row. */
export function outcomeTone(entry: HistoryEntry): HistoryOutcomeTone {
  if (entry.outcome === "error" || entry.outcome === "cancelled") {
    return "err";
  }
  if (entry.outcome === "ok" && typeof entry.status === "number") {
    if (entry.status >= 500) return "err";
    if (entry.status >= 400) return "warn";
    if (entry.status >= 200 && entry.status < 300) return "ok";
  }
  return "muted";
}

/** Whether the row has a captured EXPLAIN plan (Epic 53 Story 05).
 *  Treats empty-string + whitespace-only `plan` as absent so the
 *  truncated-fallback "all-whitespace" edge case doesn't surface a
 *  meaningless chip. Pure — exported so consumers can render the
 *  same chip in detail panels. */
export function hasPlan(entry: HistoryEntry): boolean {
  return typeof entry.plan === "string" && entry.plan.trim().length > 0;
}

/** Block label = alias when set, else `<METHOD> <URL>`. */
export function label(entry: HistoryEntry): string {
  if (entry.block_alias && entry.block_alias.trim()) {
    return entry.block_alias;
  }
  return `${entry.method} ${entry.url_canonical}`;
}

/** Compact "Xs / Xm / Xh / Xd ago" formatter. Public so consumers
 *  can reuse it in tooltips / detail panels without re-implementing
 *  the rounding rules. */
export function formatRelative(isoOrMs: string | number): string {
  const t = typeof isoOrMs === "number" ? isoOrMs : Date.parse(isoOrMs);
  if (!Number.isFinite(t)) return "—";
  const diff = Date.now() - t;
  if (diff < 0) return "now";
  const sec = Math.floor(diff / 1000);
  if (sec < 60) return `${sec}s`;
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}m`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr}h`;
  const day = Math.floor(hr / 24);
  return `${day}d`;
}

/** "12ms" / "1.2s" / "3m" depending on magnitude. */
export function formatElapsed(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  return `${Math.round(ms / 60_000)}m`;
}

function dotColor(tone: HistoryOutcomeTone): string {
  switch (tone) {
    case "ok":
      return "brand.fg";
    case "warn":
      return "warn";
    case "err":
      return "error";
    default:
      return "fg.subtle";
  }
}
