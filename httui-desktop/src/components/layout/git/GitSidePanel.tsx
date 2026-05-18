// V10.1 — VS-Code-style Source Control side panel.
//
// A right-side collapsible column (NOT a Dialog — a Dialog focus
// trap would steal keyboard input from CM6). Mirrors the
// OutlinePanel/SchemaPanel chrome. Reads the shared `useGitStore`
// via the `useGitStatus` shim so it stays in lockstep with the
// detailed pane-tab. "Details" opens the full
// pane-tab for deep-dive ("ver tudo").
//
// Open/close + persistence is owned by `useSettingsStore`
// (`gitSidePanelOpen`, user.toml `[ui].git_side_panel_open`) so the
// panel survives an app restart ("estado persiste").
//
// Cenário 2 — the commit box comes pre-filled from the commit
// template (`useSettingsStore.gitCommitTemplate`, default = built-in
// conditional). Editing wins; clearing falls back to the template.
//
// The file list, Sync button and compact history land in this same
// shell across.

import { useCallback, useEffect, useMemo, useState } from "react";
import { Box, Button, HStack, IconButton, Text } from "@chakra-ui/react";
import { LuGitBranch, LuX } from "react-icons/lu";

import { GitStatusHeader } from "@/components/layout/git/GitStatusHeader";
import { GitFileList } from "@/components/layout/git/GitFileList";
import { GitCommitForm } from "@/components/layout/git/GitCommitForm";
import { GitSyncBar } from "@/components/layout/git/GitSyncBar";
import { GitSidePanelHistory } from "@/components/layout/git/GitSidePanelHistory";
import { partitionFileChanges } from "@/components/layout/git/git-derive";
import { deriveCommitMessage } from "@/lib/blocks/commit-template";
import { useGitStatus } from "@/hooks/useGitStatus";
import { useGitCommit } from "@/hooks/useGitCommit";
import { useGitStage } from "@/hooks/useGitStage";
import { useGitSync } from "@/hooks/useGitSync";
import { useGitStore } from "@/stores/git";
import { useSettingsStore } from "@/stores/settings";
import { usePaneStore } from "@/stores/pane";
import { useWorkspaceStore } from "@/stores/workspace";

interface GitSidePanelProps {
  width: number;
  onClose: () => void;
}

export function GitSidePanel({ width, onClose }: GitSidePanelProps) {
  const vaultPath = useWorkspaceStore((s) => s.vaultPath);
  const { status } = useGitStatus(vaultPath);
  const openGitTab = usePaneStore((s) => s.openGitTab);

  const commitMessage = useGitStore((s) => s.commitMessage);
  const commitMessageDirty = useGitStore((s) => s.commitMessageDirty);
  const setCommitMessage = useGitStore((s) => s.setCommitMessage);
  const setCommitMessageFromTemplate = useGitStore(
    (s) => s.setCommitMessageFromTemplate,
  );
  const resetCommitMessage = useGitStore((s) => s.resetCommitMessage);
  const reloadLog = useGitStore((s) => s.reloadLog);
  const commits = useGitStore((s) => s.commits);
  const template = useSettingsStore((s) => s.gitCommitTemplate);
  const { commit, committing } = useGitCommit(vaultPath);
  const { toggleStage } = useGitStage(vaultPath);
  const syncState = useGitSync(vaultPath);
  const [amend, setAmend] = useState(false);

  const changedPaths = useMemo(
    () => (status?.changed ?? []).map((c) => c.path),
    [status],
  );
  const suggestion = useMemo(
    () => deriveCommitMessage(changedPaths, template),
    [changedPaths, template],
  );

  // Populate the compact history when the panel opens
  // the status poll doesn't reload the log; commit/sync do.
  useEffect(() => {
    void reloadLog();
  }, [reloadLog, vaultPath]);

  // Prefill while the draft is untouched; a hand-edit (dirty) wins.
  useEffect(() => {
    if (!commitMessageDirty && commitMessage !== suggestion) {
      setCommitMessageFromTemplate(suggestion);
    }
  }, [
    suggestion,
    commitMessageDirty,
    commitMessage,
    setCommitMessageFromTemplate,
  ]);

  const stagedCount = status
    ? partitionFileChanges(status.changed).staged.length
    : 0;

  const handleMessageChange = useCallback(
    (next: string) => {
      // Clearing the field falls back to the template.
      if (next === "") {
        resetCommitMessage();
      } else {
        setCommitMessage(next);
      }
    },
    [resetCommitMessage, setCommitMessage],
  );

  const handleCommit = useCallback(
    async (input: { message: string; amend: boolean }) => {
      await commit(input);
      setAmend(false);
      await reloadLog();
    },
    [commit, reloadLog],
  );

  return (
    <Box
      data-testid="git-side-panel"
      w={`${width}px`}
      bg="bg"
      borderLeftWidth="1px"
      borderColor="border"
      display="flex"
      flexDirection="column"
      overflow="hidden"
      flexShrink={0}
    >
      <HStack
        px={3}
        py={2}
        borderBottomWidth="1px"
        borderColor="border"
        justify="space-between"
      >
        <HStack gap={2}>
          <LuGitBranch size={14} />
          <Text
            fontSize="xs"
            fontWeight="semibold"
            color="fg.subtle"
            textTransform="uppercase"
            letterSpacing="wider"
          >
            Source Control
          </Text>
        </HStack>
        <HStack gap={1}>
          <Button
            data-testid="git-side-panel-details"
            variant="ghost"
            size="xs"
            onClick={openGitTab}
          >
            Details
          </Button>
          <IconButton
            aria-label="Close git side panel"
            variant="ghost"
            size="xs"
            onClick={onClose}
          >
            <LuX />
          </IconButton>
        </HStack>
      </HStack>

      <Box overflow="auto" flex={1}>
        {status ? (
          <>
            <GitStatusHeader status={status} />
            <GitFileList changed={status.changed} onToggleStage={toggleStage} />
            <GitCommitForm
              message={commitMessage}
              amend={amend}
              stagedCount={stagedCount}
              busy={committing}
              onMessageChange={handleMessageChange}
              onAmendChange={setAmend}
              onCommit={handleCommit}
            />
            <GitSyncBar {...syncState} />
            <GitSidePanelHistory
              vaultPath={vaultPath}
              commits={commits}
              onViewAll={openGitTab}
            />
          </>
        ) : (
          <Text
            data-testid="git-side-panel-empty"
            px={3}
            py={3}
            fontSize="11px"
            fontFamily="mono"
            color="fg.subtle"
          >
            {vaultPath ? "Not a git repository" : "No vault open"}
          </Text>
        )}
      </Box>
    </Box>
  );
}
