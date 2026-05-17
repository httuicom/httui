// Run diff side-by-side panel — Epic 47 Story 03.
//
// Pure presentational consumer of `RunDiff` from
// `lib/blocks/run-diff.ts`. Four tabs: Body / Headers / Status /
// Timing. Body uses red/green inline highlights at the changed key;
// headers row-aligned; status + timing summary chips. The consumer
// supplies the diff (already computed) so this component has no
// effects.

import { Box, Flex, Text, chakra } from "@chakra-ui/react";
import { useState } from "react";

import type {
  HeaderDiffEntry,
  JsonDiffEntry,
  RunDiff,
} from "@/lib/blocks/run-diff";

type Tab = "body" | "headers" | "status" | "timing";

export interface RunDiffPanelProps {
  diff: RunDiff;
  /** Initial tab. Default: "body" (or "status" when bodyTruncated). */
  initialTab?: Tab;
}

export function RunDiffPanel({ diff, initialTab }: RunDiffPanelProps) {
  const defaultTab: Tab =
    initialTab ?? (diff.bodyTruncated ? "status" : "body");
  const [tab, setTab] = useState<Tab>(defaultTab);

  return (
    <Box data-testid="run-diff-panel" data-tab={tab}>
      <Flex
        data-testid="run-diff-tabs"
        borderBottomWidth="1px"
        borderBottomColor="border"
        gap={0}
      >
        <TabBtn
          current={tab}
          value="body"
          onPick={setTab}
          label="Body"
          count={diff.body.length}
        />
        <TabBtn
          current={tab}
          value="headers"
          onPick={setTab}
          label="Headers"
          count={diff.headers.filter((h) => h.op !== "equal").length}
        />
        <TabBtn current={tab} value="status" onPick={setTab} label="Status" />
        <TabBtn current={tab} value="timing" onPick={setTab} label="Timing" />
      </Flex>

      {tab === "body" && <BodyTab diff={diff} />}
      {tab === "headers" && <HeadersTab entries={diff.headers} />}
      {tab === "status" && <StatusTab diff={diff} />}
      {tab === "timing" && <TimingTab diff={diff} />}
    </Box>
  );
}

function TabBtn({
  current,
  value,
  onPick,
  label,
  count,
}: {
  current: Tab;
  value: Tab;
  onPick: (t: Tab) => void;
  label: string;
  count?: number;
}) {
  const active = current === value;
  return (
    <chakra.button
      type="button"
      data-testid={`run-diff-tab-${value}`}
      data-active={active || undefined}
      onClick={() => onPick(value)}
      px={3}
      py={2}
      bg="transparent"
      borderWidth={0}
      borderBottomWidth="2px"
      borderBottomColor={active ? "brand.fg" : "transparent"}
      fontFamily="mono"
      fontSize="11px"
      color={active ? "fg" : "fg.muted"}
      cursor="pointer"
      _hover={{ color: "fg" }}
    >
      {label}
      {typeof count === "number" && count > 0 && (
        <Text as="span" ml={1} color="fg.subtle">
          ({count})
        </Text>
      )}
    </chakra.button>
  );
}

function BodyTab({ diff }: { diff: RunDiff }) {
  if (diff.bodyTruncated) {
    return (
      <Text
        data-testid="run-diff-body-truncated"
        fontSize="11px"
        color="fg.subtle"
        px={4}
        py={3}
      >
        Body diff skipped — at least one side exceeds 200 KB. Open the full file
        from{" "}
        <Text as="span" fontFamily="mono">
          .httui/runs/
        </Text>
        .
      </Text>
    );
  }
  if (diff.body.length === 0) {
    return (
      <Text
        data-testid="run-diff-body-equal"
        fontSize="11px"
        color="fg.subtle"
        px={4}
        py={3}
      >
        Bodies match.
      </Text>
    );
  }
  return (
    <Box data-testid="run-diff-body-list">
      {diff.body.map((e, i) => (
        <BodyRow key={`${e.path}-${i}`} entry={e} />
      ))}
    </Box>
  );
}

function BodyRow({ entry }: { entry: JsonDiffEntry }) {
  return (
    <Flex
      data-testid={`run-diff-body-row-${entry.path}`}
      data-op={entry.op}
      align="baseline"
      gap={2}
      px={4}
      py={1}
      borderBottomWidth="1px"
      borderBottomColor="border"
    >
      <Text
        as="span"
        fontFamily="mono"
        fontSize="10px"
        color={
          entry.op === "add"
            ? "brand.fg"
            : entry.op === "remove"
              ? "error"
              : "fg.muted"
        }
        flexShrink={0}
        w="20px"
      >
        {entry.op === "add" ? "+" : entry.op === "remove" ? "−" : "~"}
      </Text>
      <Text
        as="span"
        fontFamily="mono"
        fontSize="11px"
        color="fg"
        flex={1}
        truncate
        title={entry.path}
      >
        {entry.path}
      </Text>
      <Flex gap={2} fontFamily="mono" fontSize="10px">
        {entry.before !== undefined && (
          <Text
            as="span"
            color="error"
            data-testid={`run-diff-body-row-${entry.path}-before`}
          >
            {fmt(entry.before)}
          </Text>
        )}
        {entry.after !== undefined && (
          <Text
            as="span"
            color="brand.fg"
            data-testid={`run-diff-body-row-${entry.path}-after`}
          >
            {fmt(entry.after)}
          </Text>
        )}
      </Flex>
    </Flex>
  );
}

function HeadersTab({ entries }: { entries: ReadonlyArray<HeaderDiffEntry> }) {
  if (entries.length === 0) {
    return (
      <Text
        data-testid="run-diff-headers-empty"
        fontSize="11px"
        color="fg.subtle"
        px={4}
        py={3}
      >
        No headers on either side.
      </Text>
    );
  }
  return (
    <Box data-testid="run-diff-headers-list">
      {entries.map((h) => (
        <Flex
          key={h.key}
          data-testid={`run-diff-headers-row-${h.key}`}
          data-op={h.op}
          align="baseline"
          gap={2}
          px={4}
          py={1}
          borderBottomWidth="1px"
          borderBottomColor="border"
          opacity={h.op === "equal" ? 0.6 : 1}
        >
          <Text
            as="span"
            fontFamily="mono"
            fontSize="11px"
            color="fg"
            w="180px"
            truncate
          >
            {h.key}
          </Text>
          <Text
            as="span"
            fontFamily="mono"
            fontSize="10px"
            color="error"
            flex={1}
            truncate
            data-testid={`run-diff-headers-row-${h.key}-before`}
          >
            {h.before ?? ""}
          </Text>
          <Text
            as="span"
            fontFamily="mono"
            fontSize="10px"
            color="brand.fg"
            flex={1}
            truncate
            data-testid={`run-diff-headers-row-${h.key}-after`}
          >
            {h.after ?? ""}
          </Text>
        </Flex>
      ))}
    </Box>
  );
}

function StatusTab({ diff }: { diff: RunDiff }) {
  return (
    <Flex
      data-testid="run-diff-status"
      data-changed={diff.status.changed || undefined}
      gap={3}
      px={4}
      py={3}
      align="baseline"
      fontFamily="mono"
      fontSize="11px"
    >
      <Text color="fg.subtle">A:</Text>
      <Text color={diff.status.changed ? "error" : "fg"}>
        {diff.status.before ?? "—"}
      </Text>
      <Text color="fg.subtle">B:</Text>
      <Text color={diff.status.changed ? "brand.fg" : "fg"}>
        {diff.status.after ?? "—"}
      </Text>
    </Flex>
  );
}

function TimingTab({ diff }: { diff: RunDiff }) {
  const delta = diff.timing.deltaMs;
  return (
    <Flex
      data-testid="run-diff-timing"
      gap={3}
      px={4}
      py={3}
      align="baseline"
      fontFamily="mono"
      fontSize="11px"
    >
      <Text color="fg.subtle">A:</Text>
      <Text>{diff.timing.before ?? "—"}ms</Text>
      <Text color="fg.subtle">B:</Text>
      <Text>{diff.timing.after ?? "—"}ms</Text>
      {delta !== undefined && (
        <Text
          color={delta > 0 ? "error" : delta < 0 ? "brand.fg" : "fg.muted"}
          data-testid="run-diff-timing-delta"
        >
          {delta > 0 ? `+${delta}` : delta}ms
        </Text>
      )}
    </Flex>
  );
}

function fmt(v: unknown): string {
  if (v === null) return "null";
  if (typeof v === "string") return JSON.stringify(v);
  if (typeof v === "number" || typeof v === "boolean") return String(v);
  try {
    return JSON.stringify(v);
  } catch {
    return String(v);
  }
}
