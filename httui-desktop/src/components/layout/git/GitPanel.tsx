// V10 — Git panel shell with Status / Log / Audit tabs.
//
// Composes the Epic 48 carry sub-components (GitStatusHeader,
// GitFileList, GitCommitForm, GitCommitDiffViewer, GitLogList,
// GitAuditHeader) into a tabbed surface. Purely presentational and
// controlled: the consumer (`GitPanelContainer`) owns data fetching,
// the active tab, the commit-form state, and the dispatch callbacks.
// Audit tab is "log, no action-type filters" per the V10 decision
// (filters deferred to v1.x).

import { Box, Flex, Text, chakra } from "@chakra-ui/react";

import type { CommitInfo, GitFileChange, GitStatus } from "@/lib/tauri/git";

import { GitAuditHeader } from "./GitAuditHeader";
import { GitCommitDiffViewer } from "./GitCommitDiffViewer";
import { GitCommitForm } from "./GitCommitForm";
import { GitFileList } from "./GitFileList";
import { GitLogList } from "./GitLogList";
import { GitStatusHeader } from "./GitStatusHeader";

export type GitPanelTab = "status" | "log" | "audit";

export const GIT_PANEL_TABS: ReadonlyArray<{
  id: GitPanelTab;
  label: string;
}> = [
  { id: "status", label: "Status" },
  { id: "log", label: "Log" },
  { id: "audit", label: "Audit" },
];

export interface GitPanelProps {
  status: GitStatus | null;
  commits: ReadonlyArray<CommitInfo>;
  /** Active tab. Controlled by the consumer. Defaults to Status. */
  activeTab?: GitPanelTab;
  onSelectTab?: (tab: GitPanelTab) => void;
  /** Path of the file currently shown in the diff side-panel, if any. */
  selectedFilePath?: string | null;
  /** SHA of the commit currently inspected, if any. */
  selectedCommitSha?: string | null;
  onToggleStage?: (file: GitFileChange) => void;
  onSelectFile?: (file: GitFileChange) => void;
  onSelectCommit?: (commit: CommitInfo) => void;
  onAuditLearnMore?: () => void;
  // --- Commit form (cenário 2) ---
  stagedCount?: number;
  commitMessage?: string;
  commitAmend?: boolean;
  committing?: boolean;
  onCommitMessageChange?: (next: string) => void;
  onCommitAmendChange?: (next: boolean) => void;
  onCommit?: (input: { message: string; amend: boolean }) => void;
  // --- Diff inspector (cenário 2 preview / cenário 3 commit) ---
  /** `undefined` hides the inspector; `null` shows "loading"; a
   *  string renders the unified diff. */
  diff?: string | null;
  diffShortSha?: string | null;
  diffSubject?: string | null;
}

const TabButton = chakra("button");

export function GitPanel({
  status,
  commits,
  activeTab = "status",
  onSelectTab,
  selectedFilePath,
  selectedCommitSha,
  onToggleStage,
  onSelectFile,
  onSelectCommit,
  onAuditLearnMore,
  stagedCount = 0,
  commitMessage = "",
  commitAmend = false,
  committing,
  onCommitMessageChange,
  onCommitAmendChange,
  onCommit,
  diff,
  diffShortSha,
  diffSubject,
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

  const showCommitForm =
    !!onCommit && !!onCommitMessageChange && !!onCommitAmendChange;

  return (
    <Flex
      data-testid="git-panel"
      data-clean={status.clean || undefined}
      data-active-tab={activeTab}
      direction="column"
      h="100%"
      minH={0}
    >
      <Flex
        data-testid="git-panel-tabs"
        flexShrink={0}
        borderBottomWidth="1px"
        borderBottomColor="border"
        bg="bg.subtle"
      >
        {GIT_PANEL_TABS.map((t) => {
          const active = t.id === activeTab;
          return (
            <TabButton
              key={t.id}
              type="button"
              data-testid={`git-tab-${t.id}`}
              data-active={active || undefined}
              aria-selected={active}
              onClick={() => onSelectTab?.(t.id)}
              px={3}
              py={2}
              fontFamily="mono"
              fontSize="11px"
              color={active ? "fg" : "fg.subtle"}
              bg={active ? "bg" : "transparent"}
              borderBottomWidth="2px"
              borderBottomColor={active ? "brand.fg" : "transparent"}
              cursor="pointer"
              _hover={active ? undefined : { color: "fg.muted" }}
            >
              {t.label}
            </TabButton>
          );
        })}
      </Flex>

      {activeTab === "status" && (
        <Flex direction="column" flex="1 1 auto" minH={0}>
          <GitStatusHeader status={status} />
          <Box
            data-testid="git-panel-section-working-tree"
            flex="1 1 auto"
            minH={0}
            overflow="auto"
          >
            <SectionLabel>Working tree</SectionLabel>
            <GitFileList
              changed={status.changed}
              selectedPath={selectedFilePath}
              onToggleStage={onToggleStage}
              onSelect={onSelectFile}
            />
          </Box>
          {showCommitForm && (
            <GitCommitForm
              message={commitMessage}
              amend={commitAmend}
              stagedCount={stagedCount}
              busy={committing}
              onMessageChange={onCommitMessageChange!}
              onAmendChange={onCommitAmendChange!}
              onCommit={onCommit!}
            />
          )}
          {diff !== undefined && (
            <Box
              data-testid="git-panel-section-diff"
              flexShrink={0}
              maxH="40%"
              overflow="auto"
              borderTopWidth="1px"
              borderTopColor="border"
            >
              <GitCommitDiffViewer
                diff={diff}
                shortSha={diffShortSha}
                subject={diffSubject}
              />
            </Box>
          )}
        </Flex>
      )}

      {activeTab === "log" && (
        <Box
          data-testid="git-panel-section-log"
          flex="1 1 auto"
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
      )}

      {activeTab === "audit" && (
        <Box
          data-testid="git-panel-section-audit"
          flex="1 1 auto"
          minH={0}
          overflow="auto"
        >
          <GitAuditHeader onLearnMore={onAuditLearnMore} />
          <GitLogList
            commits={commits}
            selectedSha={selectedCommitSha}
            onSelect={onSelectCommit}
          />
        </Box>
      )}
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
