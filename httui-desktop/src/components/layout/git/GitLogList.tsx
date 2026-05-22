import { Box, Flex, Text } from "@chakra-ui/react";

import type { CommitInfo } from "@/lib/tauri/git";

import { authorInitials, relativeTime } from "./git-derive";

export interface GitLogListProps {
  commits: ReadonlyArray<CommitInfo>;
  /** SHA of the currently inspected commit, if any. */
  selectedSha?: string | null;
  onSelect?: (commit: CommitInfo) => void;
}

export function GitLogList({
  commits,
  selectedSha,
  onSelect,
}: GitLogListProps) {
  if (commits.length === 0) {
    return (
      <Text
        data-testid="git-log-list-empty"
        fontSize="11px"
        color="fg.subtle"
        px={3}
        py={4}
      >
        No commits yet.
      </Text>
    );
  }

  return (
    <Box data-testid="git-log-list" data-count={commits.length}>
      {commits.map((c) => (
        <Row
          key={c.sha}
          commit={c}
          selected={selectedSha === c.sha}
          onSelect={onSelect}
        />
      ))}
    </Box>
  );
}

function Row({
  commit,
  selected,
  onSelect,
}: {
  commit: CommitInfo;
  selected: boolean;
  onSelect?: (c: CommitInfo) => void;
}) {
  const initials = authorInitials(commit);
  const when = relativeTime(commit.timestamp);
  return (
    <Flex
      data-testid={`git-log-row-${commit.short_sha}`}
      data-selected={selected || undefined}
      align="center"
      gap={2}
      px={3}
      py={1}
      bg={selected ? "bg.muted" : undefined}
      _hover={{ bg: "bg.muted" }}
      cursor={onSelect ? "pointer" : undefined}
      onClick={() => onSelect?.(commit)}
    >
      <Text
        as="span"
        fontFamily="mono"
        fontSize="10px"
        color="fg.subtle"
        flexShrink={0}
        w="56px"
      >
        {commit.short_sha}
      </Text>
      <Text
        as="span"
        fontFamily="mono"
        fontSize="10px"
        color="fg.muted"
        flexShrink={0}
        w="28px"
        textAlign="center"
        title={`${commit.author_name} <${commit.author_email}>`}
        data-testid={`git-log-row-${commit.short_sha}-initials`}
      >
        {initials}
      </Text>
      <Text
        as="span"
        fontFamily="mono"
        fontSize="11px"
        color="fg"
        flex={1}
        truncate
        title={commit.subject}
      >
        {commit.subject}
      </Text>
      <Text
        as="span"
        fontFamily="mono"
        fontSize="10px"
        color="fg.subtle"
        flexShrink={0}
        title={new Date(commit.timestamp * 1000).toISOString()}
      >
        {when}
      </Text>
    </Flex>
  );
}
