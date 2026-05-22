import { Box, Flex, Text } from "@chakra-ui/react";

import type { CommitInfo, GitStatus, Remote } from "@/lib/tauri/git";

import {
  relativeTime,
  summarizeBranch,
  summarizeChangeCounts,
} from "./git-derive";

export interface GitMetricsStripProps {
  status: GitStatus;
  commits: ReadonlyArray<CommitInfo>;
  remotes: ReadonlyArray<Remote>;
  /** Epoch ms of the last successful sync, or null. */
  lastSyncAt: number | null;
}

function Cell({
  label,
  value,
  testId,
  title,
}: {
  label: string;
  value: string;
  testId: string;
  title?: string;
}) {
  return (
    <Flex direction="column" gap={0} minW={0} data-testid={testId}>
      <Text
        fontSize="9px"
        fontFamily="mono"
        textTransform="uppercase"
        letterSpacing="wider"
        color="fg.subtle"
      >
        {label}
      </Text>
      <Text
        fontSize="11px"
        fontFamily="mono"
        color="fg"
        truncate
        title={title ?? value}
      >
        {value}
      </Text>
    </Flex>
  );
}

export function GitMetricsStrip({
  status,
  commits,
  remotes,
  lastSyncAt,
}: GitMetricsStripProps) {
  const branch = summarizeBranch(status);
  const counts = summarizeChangeCounts(status.changed);
  const head = commits[0] ?? null;
  const remoteUrl = remotes[0]?.url ?? "—";

  return (
    <Flex
      data-testid="git-metrics-strip"
      flexShrink={0}
      align="stretch"
      gap={4}
      px={3}
      py={2}
      borderBottomWidth="1px"
      borderBottomColor="border"
      bg="bg.subtle"
      overflowX="auto"
    >
      <Cell testId="git-metric-branch" label="Branch" value={branch.label} />
      <Cell
        testId="git-metric-upstream"
        label="Upstream"
        value={branch.upstream ?? "none"}
      />
      <Cell
        testId="git-metric-aheadbehind"
        label="Ahead / Behind"
        value={`↑${branch.ahead} ↓${branch.behind}`}
      />
      <Cell
        testId="git-metric-changes"
        label="Changes"
        value={`M${counts.modified} A${counts.added} D${counts.deleted} ?${counts.untracked} U${counts.conflicted}`}
      />
      <Cell
        testId="git-metric-lastcommit"
        label="Last commit"
        value={
          head ? `${head.author_name} · ${relativeTime(head.timestamp)}` : "—"
        }
        title={head?.subject}
      />
      <Cell
        testId="git-metric-lastsync"
        label="Last sync"
        value={
          lastSyncAt === null
            ? "never"
            : relativeTime(Math.floor(lastSyncAt / 1000))
        }
      />
      <Box flex={1} minW="40px" />
      <Cell testId="git-metric-remote" label="Remote" value={remoteUrl} />
    </Flex>
  );
}
