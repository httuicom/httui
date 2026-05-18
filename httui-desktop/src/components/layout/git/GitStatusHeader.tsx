// top section of the Git panel: branch label,
// upstream, ahead/behind chips, dirty count.
//
// Pure presentational. Consumer fetches `GitStatus` via
// `gitStatus(vaultPath)` and passes it as a prop.

import { Box, Flex, Text } from "@chakra-ui/react";

import type { GitStatus } from "@/lib/tauri/git";

import { summarizeBranch } from "./git-derive";

export interface GitStatusHeaderProps {
  status: GitStatus;
}

export function GitStatusHeader({ status }: GitStatusHeaderProps) {
  const summary = summarizeBranch(status);
  const dirtyCount = status.changed.length;

  return (
    <Flex
      data-testid="git-status-header"
      data-clean={status.clean || undefined}
      data-detached={status.branch === null || undefined}
      data-no-upstream={summary.noUpstream || undefined}
      align="center"
      gap={2}
      px={3}
      py={2}
      borderBottomWidth="1px"
      borderBottomColor="border"
      bg="bg.subtle"
    >
      <Text
        as="span"
        fontFamily="mono"
        fontSize="11px"
        fontWeight="600"
        color="fg"
        flexShrink={0}
        data-testid="git-status-header-branch"
      >
        {summary.label}
      </Text>
      {summary.upstream && (
        <Text
          as="span"
          fontFamily="mono"
          fontSize="10px"
          color="fg.subtle"
          flexShrink={0}
          data-testid="git-status-header-upstream"
        >
          → {summary.upstream}
        </Text>
      )}
      {summary.ahead > 0 && (
        <Chip
          testId="git-status-header-ahead"
          tone="brand.fg"
          label={`↑ ${summary.ahead}`}
        />
      )}
      {summary.behind > 0 && (
        <Chip
          testId="git-status-header-behind"
          tone="warn"
          label={`↓ ${summary.behind}`}
        />
      )}
      <Box flex={1} />
      {!status.clean && (
        <Text
          as="span"
          fontFamily="mono"
          fontSize="10px"
          color="warn"
          flexShrink={0}
          data-testid="git-status-header-dirty"
        >
          {dirtyCount} {dirtyCount === 1 ? "change" : "changes"}
        </Text>
      )}
      {status.clean && (
        <Text
          as="span"
          fontFamily="mono"
          fontSize="10px"
          color="fg.subtle"
          flexShrink={0}
          data-testid="git-status-header-clean"
        >
          clean
        </Text>
      )}
    </Flex>
  );
}

function Chip({
  testId,
  tone,
  label,
}: {
  testId: string;
  tone: "brand.fg" | "warn";
  label: string;
}) {
  return (
    <Text
      as="span"
      data-testid={testId}
      data-tone={tone}
      fontFamily="mono"
      fontSize="10px"
      color={tone === "brand.fg" ? "brand.fg" : "warn"}
      flexShrink={0}
    >
      {label}
    </Text>
  );
}
