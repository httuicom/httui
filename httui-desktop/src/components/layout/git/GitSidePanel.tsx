// V10.1 cenário 1 — VS-Code-style Source Control side panel.
//
// A right-side collapsible column (NOT a Dialog — a Dialog focus
// trap would steal keyboard input from CM6). Mirrors the
// OutlinePanel/SchemaPanel chrome. Reads the shared `useGitStore`
// via the `useGitStatus` shim so it stays in lockstep with the
// detailed pane-tab (cenário 7). "Details" opens the full V10
// pane-tab for deep-dive (cenário 4 "ver tudo").
//
// Open/close + persistence is owned by `useSettingsStore`
// (`gitSidePanelOpen`, user.toml `[ui].git_side_panel_open`) so the
// panel survives an app restart (cenário 1 "estado persiste").
//
// The commit box, file list, sync button and compact history land
// in this same shell across cenários 2–6.

import { Box, Button, HStack, IconButton, Text } from "@chakra-ui/react";
import { LuGitBranch, LuX } from "react-icons/lu";

import { GitStatusHeader } from "@/components/layout/git/GitStatusHeader";
import { useGitStatus } from "@/hooks/useGitStatus";
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
          <GitStatusHeader status={status} />
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
