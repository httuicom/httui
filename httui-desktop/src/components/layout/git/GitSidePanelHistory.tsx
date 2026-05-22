import { useCallback, useState } from "react";
import { Box, Button, Flex, Text } from "@chakra-ui/react";

import { GitLogList } from "@/components/layout/git/GitLogList";
import { GitCommitDiffViewer } from "@/components/layout/git/GitCommitDiffViewer";
import { gitDiff, type CommitInfo } from "@/lib/tauri/git";

interface GitSidePanelHistoryProps {
  vaultPath: string | null;
  commits: ReadonlyArray<CommitInfo>;
  onViewAll: () => void;
  /** Recent commits shown in the compact list. */
  limit?: number;
}

export function GitSidePanelHistory({
  vaultPath,
  commits,
  onViewAll,
  limit = 10,
}: GitSidePanelHistoryProps) {
  const [selected, setSelected] = useState<CommitInfo | null>(null);
  const [diff, setDiff] = useState<string | null>(null);

  const handleSelect = useCallback(
    async (commit: CommitInfo) => {
      if (selected?.sha === commit.sha) {
        setSelected(null);
        setDiff(null);
        return;
      }
      setSelected(commit);
      setDiff(null);
      if (!vaultPath) return;
      try {
        setDiff(await gitDiff(vaultPath, commit.sha));
      } catch {
        setDiff("");
      }
    },
    [selected, vaultPath],
  );

  const recent = commits.slice(0, limit);

  return (
    <Box
      data-testid="git-side-panel-history"
      borderTopWidth="1px"
      borderTopColor="border"
    >
      <Flex align="center" justify="space-between" px={3} py={1} bg="bg.muted">
        <Text
          fontFamily="mono"
          fontSize="10px"
          textTransform="uppercase"
          color="fg.subtle"
        >
          History
        </Text>
        <Button
          data-testid="git-side-panel-history-view-all"
          variant="ghost"
          size="xs"
          onClick={onViewAll}
        >
          View all
        </Button>
      </Flex>
      <GitLogList
        commits={recent}
        selectedSha={selected?.sha ?? null}
        onSelect={handleSelect}
      />
      {selected && (
        <GitCommitDiffViewer
          shortSha={selected.short_sha}
          subject={selected.subject}
          diff={diff}
        />
      )}
    </Box>
  );
}
