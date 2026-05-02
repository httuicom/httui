// Block run-history menu — Epic 47 Story 04.
//
// Presentational. Lists the last N runs of a single block (already
// fetched by the consumer via `listBlockHistory`) with per-row
// View / Diff-with-current / Diff-with… actions. The consumer wires
// the handlers to the existing run cache + `<RunDiffPanel>`.

import { Box, Flex, Text } from "@chakra-ui/react";

import { Btn } from "@/components/atoms";
import type { HistoryEntry } from "@/lib/tauri/commands";

export interface RunHistoryMenuProps {
  entries: ReadonlyArray<HistoryEntry>;
  /** Optional id of the run currently rendered as "live" — Diff-with-current is hidden for it. */
  liveRunId?: number | null;
  onView?: (entry: HistoryEntry) => void;
  onDiffWithCurrent?: (entry: HistoryEntry) => void;
  /** "Diff with…" picker — the consumer pops a second selector. */
  onDiffWithPick?: (entry: HistoryEntry) => void;
}

export function RunHistoryMenu({
  entries,
  liveRunId,
  onView,
  onDiffWithCurrent,
  onDiffWithPick,
}: RunHistoryMenuProps) {
  if (entries.length === 0) {
    return (
      <Text
        data-testid="run-history-menu-empty"
        fontSize="11px"
        color="fg.subtle"
        px={4}
        py={3}
      >
        No runs yet. Run the block to start the history.
      </Text>
    );
  }

  return (
    <Box data-testid="run-history-menu" data-count={entries.length}>
      {entries.map((entry) => (
        <Row
          key={entry.id}
          entry={entry}
          isLive={entry.id === liveRunId}
          onView={onView}
          onDiffWithCurrent={onDiffWithCurrent}
          onDiffWithPick={onDiffWithPick}
        />
      ))}
    </Box>
  );
}

function Row({
  entry,
  isLive,
  onView,
  onDiffWithCurrent,
  onDiffWithPick,
}: {
  entry: HistoryEntry;
  isLive: boolean;
  onView?: (e: HistoryEntry) => void;
  onDiffWithCurrent?: (e: HistoryEntry) => void;
  onDiffWithPick?: (e: HistoryEntry) => void;
}) {
  const ok = entry.outcome === "success";
  return (
    <Flex
      data-testid={`run-history-row-${entry.id}`}
      data-live={isLive || undefined}
      data-outcome={entry.outcome}
      align="center"
      gap={2}
      px={4}
      py={2}
      borderBottomWidth="1px"
      borderBottomColor="border"
    >
      <Text
        as="span"
        fontFamily="mono"
        fontSize="10px"
        color={ok ? "accent" : "error"}
        flexShrink={0}
        w="40px"
      >
        {entry.method}
      </Text>
      <Text
        as="span"
        fontFamily="mono"
        fontSize="10px"
        color={ok ? "fg.muted" : "error"}
        flexShrink={0}
        w="40px"
      >
        {entry.status ?? "—"}
      </Text>
      <Text
        as="span"
        fontFamily="mono"
        fontSize="11px"
        color="fg"
        flex={1}
        truncate
        title={entry.url_canonical}
      >
        {entry.url_canonical}
      </Text>
      <Text
        as="span"
        fontFamily="mono"
        fontSize="10px"
        color="fg.subtle"
        flexShrink={0}
        title={entry.ran_at}
      >
        {formatRanAt(entry.ran_at)}
      </Text>
      <Flex gap={1} flexShrink={0}>
        {onView && (
          <Btn
            variant="ghost"
            data-testid={`run-history-row-${entry.id}-view`}
            onClick={() => onView(entry)}
          >
            View
          </Btn>
        )}
        {onDiffWithCurrent && !isLive && (
          <Btn
            variant="ghost"
            data-testid={`run-history-row-${entry.id}-diff-current`}
            onClick={() => onDiffWithCurrent(entry)}
          >
            Diff
          </Btn>
        )}
        {onDiffWithPick && (
          <Btn
            variant="ghost"
            data-testid={`run-history-row-${entry.id}-diff-pick`}
            onClick={() => onDiffWithPick(entry)}
          >
            Diff…
          </Btn>
        )}
      </Flex>
    </Flex>
  );
}

/** Best-effort relative formatter. The Rust side stores ISO-8601;
 * fall back to the raw string when parsing fails. */
function formatRanAt(iso: string): string {
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return iso;
  const diffSec = Math.max(0, Math.floor((Date.now() - t) / 1000));
  if (diffSec < 60) return `${diffSec}s ago`;
  if (diffSec < 3600) return `${Math.floor(diffSec / 60)}m ago`;
  if (diffSec < 86400) return `${Math.floor(diffSec / 3600)}h ago`;
  return `${Math.floor(diffSec / 86400)}d ago`;
}
