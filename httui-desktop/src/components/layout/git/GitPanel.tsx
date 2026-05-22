import { Box, Flex, Text, chakra } from "@chakra-ui/react";

import { Btn } from "@/components/atoms";
import type {
  CommitInfo,
  ConflictVersions,
  GitFileChange,
  GitStatus,
  Remote,
} from "@/lib/tauri/git";

import { GitCommitDiffViewer } from "./GitCommitDiffViewer";
import { GitMetricsStrip } from "./GitMetricsStrip";
import { GitCommitForm } from "./GitCommitForm";
import { GitConflictBanner } from "./GitConflictBanner";
import { GitConflictResolver } from "./GitConflictResolver";
import { GitFileList } from "./GitFileList";
import { GitLogFilter } from "./GitLogFilter";
import { GitLogList } from "./GitLogList";
import { GitStatusHeader } from "./GitStatusHeader";
import { GitSyncButtons, type SyncOp } from "./GitSyncButtons";
import type { LogFilterState } from "./git-log-filter";

export type GitPanelTab = "status" | "log";

export const GIT_PANEL_TABS: ReadonlyArray<{
  id: GitPanelTab;
  label: string;
}> = [
  { id: "status", label: "Status" },
  { id: "log", label: "Log" },
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
  // --- Commit form ---------------
  stagedCount?: number;
  commitMessage?: string;
  commitAmend?: boolean;
  committing?: boolean;
  onCommitMessageChange?: (next: string) => void;
  onCommitAmendChange?: (next: boolean) => void;
  onCommit?: (input: { message: string; amend: boolean }) => void;
  // --- Diff inspector (preview / commit) -----------------------
  /** `undefined` hides the inspector; `null` shows "loading"; a
   *  string renders the unified diff. */
  diff?: string | null;
  diffShortSha?: string | null;
  diffSubject?: string | null;
  // --- Log filter ---------------
  logFilter?: LogFilterState;
  onLogFilterChange?: (next: LogFilterState) => void;
  // --- Sync toolbar ---------------
  syncInFlight?: SyncOp | null;
  hasRemote?: boolean;
  onFetch?: () => void;
  onPull?: () => void;
  onPush?: () => void;
  onConfigureRemote?: () => void;
  /** When set, the no-upstream confirm banner is shown for this
   * branch. */
  upstreamPrompt?: { branch: string; remote: string } | null;
  onConfirmSetUpstream?: () => void;
  onCancelSetUpstream?: () => void;
  // --- Conflict resolution ---------------
  conflicts?: ReadonlyArray<string>;
  conflictBusy?: boolean;
  onOpenConflict?: (path: string) => void;
  onAcceptYours?: (path: string) => void;
  onAcceptTheirs?: (path: string) => void;
  /** When set, the 3-way resolver takes over the panel body. */
  resolver?: { path: string; versions: ConflictVersions } | null;
  onResolveMerged?: (path: string, merged: string) => void;
  onCancelResolver?: () => void;
  /** Right-aligned toolbar slot (mounts ShareMenu). */
  toolbarExtra?: React.ReactNode;
  // --- Metrics strip ---------------------
  remotes?: ReadonlyArray<Remote>;
  /** Epoch ms of the last successful sync, or null. */
  lastSyncAt?: number | null;
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
  logFilter,
  onLogFilterChange,
  syncInFlight = null,
  hasRemote = true,
  onFetch,
  onPull,
  onPush,
  onConfigureRemote,
  upstreamPrompt,
  onConfirmSetUpstream,
  onCancelSetUpstream,
  conflicts = [],
  conflictBusy,
  onOpenConflict,
  onAcceptYours,
  onAcceptTheirs,
  resolver,
  onResolveMerged,
  onCancelResolver,
  toolbarExtra,
  remotes = [],
  lastSyncAt = null,
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

      <GitMetricsStrip
        status={status}
        commits={commits}
        remotes={remotes}
        lastSyncAt={lastSyncAt}
      />

      {(onFetch || onPull || onPush || toolbarExtra) && (
        <Flex
          align="center"
          flexShrink={0}
          borderBottomWidth="1px"
          borderBottomColor="border"
        >
          <Box flex={1} minW={0}>
            {(onFetch || onPull || onPush) && (
              <GitSyncButtons
                inFlight={syncInFlight}
                hasRemote={hasRemote}
                onFetch={onFetch}
                onPull={onPull}
                onPush={onPush}
                onConfigureRemote={onConfigureRemote}
              />
            )}
          </Box>
          {toolbarExtra && (
            <Box flexShrink={0} px={2}>
              {toolbarExtra}
            </Box>
          )}
        </Flex>
      )}

      {upstreamPrompt && (
        <Box
          data-testid="git-upstream-prompt"
          flexShrink={0}
          px={3}
          py={2}
          bg="bg.muted"
          borderBottomWidth="1px"
          borderBottomColor="border"
        >
          <Text fontSize="11px" color="fg" mb={2}>
            Branch <strong>{upstreamPrompt.branch}</strong> has no upstream. Set
            upstream to{" "}
            <strong>
              {upstreamPrompt.remote}/{upstreamPrompt.branch}
            </strong>{" "}
            and push?
          </Text>
          <Flex gap={2}>
            <Btn
              data-testid="git-upstream-prompt-confirm"
              variant="primary"
              onClick={onConfirmSetUpstream}
            >
              Set upstream &amp; push
            </Btn>
            <Btn
              data-testid="git-upstream-prompt-cancel"
              variant="ghost"
              onClick={onCancelSetUpstream}
            >
              Cancel
            </Btn>
          </Flex>
        </Box>
      )}

      {resolver && onResolveMerged && onCancelResolver && (
        <Box
          data-testid="git-panel-resolver"
          flex="1 1 auto"
          minH={0}
          overflow="hidden"
        >
          <GitConflictResolver
            path={resolver.path}
            versions={resolver.versions}
            busy={conflictBusy}
            onResolve={onResolveMerged}
            onCancel={onCancelResolver}
          />
        </Box>
      )}

      {!resolver && activeTab === "status" && (
        <Flex direction="column" flex="1 1 auto" minH={0}>
          <GitStatusHeader status={status} />
          {conflicts.length > 0 && (
            <Box px={3} pt={2} flexShrink={0}>
              <GitConflictBanner
                conflicts={conflicts}
                busy={conflictBusy}
                onOpenDiff={onOpenConflict}
                onAcceptYours={onAcceptYours}
                onAcceptTheirs={onAcceptTheirs}
              />
            </Box>
          )}
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
          <DiffSection
            diff={diff}
            shortSha={diffShortSha}
            subject={diffSubject}
          />
        </Flex>
      )}

      {!resolver && activeTab === "log" && (
        <Flex direction="column" flex="1 1 auto" minH={0}>
          {logFilter && onLogFilterChange && (
            <GitLogFilter state={logFilter} onChange={onLogFilterChange} />
          )}
          <Box
            data-testid="git-panel-section-log"
            flex="1 1 auto"
            minH={0}
            overflow="auto"
          >
            <GitLogList
              commits={commits}
              selectedSha={selectedCommitSha}
              onSelect={onSelectCommit}
            />
          </Box>
          <DiffSection
            diff={diff}
            shortSha={diffShortSha}
            subject={diffSubject}
          />
        </Flex>
      )}
    </Flex>
  );
}

/** Shared diff inspector slot — Status tab (working diff) and Log
 *  tab (commit diff). `undefined` diff renders nothing. */
function DiffSection({
  diff,
  shortSha,
  subject,
}: {
  diff?: string | null;
  shortSha?: string | null;
  subject?: string | null;
}) {
  if (diff === undefined) return null;
  return (
    <Box
      data-testid="git-panel-section-diff"
      flexShrink={0}
      maxH="40%"
      overflow="auto"
      borderTopWidth="1px"
      borderTopColor="border"
    >
      <GitCommitDiffViewer diff={diff} shortSha={shortSha} subject={subject} />
    </Box>
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
