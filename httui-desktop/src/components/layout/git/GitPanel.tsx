// Epic 48 Story 01 — Git panel layout.
//
// Three-section vertical Flex: Status header / Working tree / Log.
// The panel is purely presentational — the consumer (future
// `useGitPanel` hook + sidebar mount, parked as Story 02 / Epic 30a
// sweep work) fetches `gitStatus`/`gitLog`/`gitBranchList` and passes
// the values down. The panel itself does not call Tauri.

import { Box, Flex, Text } from "@chakra-ui/react";

import type { CommitInfo, GitFileChange, GitStatus } from "@/lib/tauri/git";

import { GitFileList } from "./GitFileList";
import { GitLogList } from "./GitLogList";
import { GitStatusHeader } from "./GitStatusHeader";

export interface GitPanelProps {
  status: GitStatus | null;
  commits: ReadonlyArray<CommitInfo>;
  /** Path of the file currently shown in the diff side-panel, if any. */
  selectedFilePath?: string | null;
  /** SHA of the commit currently inspected, if any. */
  selectedCommitSha?: string | null;
  onToggleStage?: (file: GitFileChange) => void;
  onSelectFile?: (file: GitFileChange) => void;
  onSelectCommit?: (commit: CommitInfo) => void;
}

export function GitPanel({
  status,
  commits,
  selectedFilePath,
  selectedCommitSha,
  onToggleStage,
  onSelectFile,
  onSelectCommit,
}: GitPanelProps) {
  if (status === null) {
    return (
      <Box data-testid="git-panel" data-loading="true" px={3} py={4}>
        <Text fontSize="11px" color="fg.subtle">
          Loading git state…
        </Text>
      </Box>
    );
  }

  return (
    <Flex
      data-testid="git-panel"
      data-clean={status.clean || undefined}
      direction="column"
      h="100%"
      minH={0}
    >
      <GitStatusHeader status={status} />

      <Box
        data-testid="git-panel-section-working-tree"
        flex="1 1 60%"
        minH={0}
        overflow="auto"
        borderBottomWidth="1px"
        borderBottomColor="border"
      >
        <SectionLabel>Working tree</SectionLabel>
        <GitFileList
          changed={status.changed}
          selectedPath={selectedFilePath}
          onToggleStage={onToggleStage}
          onSelect={onSelectFile}
        />
      </Box>

      <Box
        data-testid="git-panel-section-log"
        flex="1 1 40%"
        minH={0}
        overflow="auto"
      >
        <SectionLabel>Log</SectionLabel>
        <GitLogList
          commits={commits}
          selectedSha={selectedCommitSha}
          onSelect={onSelectCommit}
        />
      </Box>
    </Flex>
  );
}

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <Text
      as="div"
      fontFamily="mono"
      fontSize="10px"
      textTransform="uppercase"
      color="fg.subtle"
      px={3}
      py={1}
      bg="bg.subtle"
      borderBottomWidth="1px"
      borderBottomColor="border"
    >
      {children}
    </Text>
  );
}
