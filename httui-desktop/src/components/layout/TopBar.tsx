// Workbench top bar — canvas §4 (36px tall).
//
// Layout:
//   [Sidebar toggle][Brand][Breadcrumb] ━━━ [SegmentedEnvSwitcher]
//   [SearchPlaceholder ⌘K] [BranchButton] [Schema][Chat][Settings]
//
// The Sidebar/Chat/Schema/Settings toggles are functional necessities
// and live on the right edge. Brand wordmark is centered with the
// Tauri drag region in mind (`pl="80px"` reserves space for macOS
// traffic-light buttons).
//
// Search ⌘K re-dispatches Cmd+P so the existing keyboard route picks
// it up. Branch is read-only awaiting V10. The "Run all" button was
// dropped per user decision on 2026-05-01 (V2 / cenário 1).

import { Box, HStack, IconButton, chakra } from "@chakra-ui/react";
import {
  LuMenu,
  LuGitBranch,
  LuSearch,
  LuMessageSquare,
  LuSettings,
  LuDatabase,
  LuListTree,
  LuClock,
} from "react-icons/lu";

import { Brand } from "@/components/layout/topbar/Brand";
import { BreadcrumbNav } from "@/components/layout/topbar/BreadcrumbNav";
import { SegmentedEnvSwitcher } from "@/components/layout/topbar/SegmentedEnvSwitcher";
import { Btn, Kbd } from "@/components/atoms";
import { useWorkspace } from "@/contexts/WorkspaceContext";
import { useGitStatus } from "@/hooks/useGitStatus";
import { useSettingsStore } from "@/stores/settings";
import {
  usePaneStore,
  selectActiveTabPath,
  selectActiveTabUnsaved,
} from "@/stores/pane";

const SearchTrigger = chakra("button");

interface TopBarProps {
  sidebarOpen: boolean;
  onToggleSidebar: () => void;
  chatOpen: boolean;
  onToggleChat: () => void;
  schemaPanelOpen: boolean;
  onToggleSchemaPanel: () => void;
  outlinePanelOpen?: boolean;
  onToggleOutlinePanel?: () => void;
  historyPanelOpen?: boolean;
  onToggleHistoryPanel?: () => void;
  /** Optional override for tests / consumer-driven control. Defaults
   * to dispatching a `Mod-p` keyboard event so existing handlers
   * pick it up. */
  onSearch?: () => void;
}

function defaultSearchTrigger() {
  // Re-dispatch the Cmd+P shortcut so the existing keyboard hook
  // route handles it. Avoids couping the TopBar to QuickOpen state.
  if (typeof window === "undefined") return;
  const ev = new KeyboardEvent("keydown", {
    key: "p",
    code: "KeyP",
    metaKey: true,
    bubbles: true,
  });
  window.dispatchEvent(ev);
}

export function TopBar({
  sidebarOpen,
  onToggleSidebar,
  chatOpen,
  onToggleChat,
  schemaPanelOpen,
  onToggleSchemaPanel,
  outlinePanelOpen,
  onToggleOutlinePanel,
  historyPanelOpen,
  onToggleHistoryPanel,
  onSearch = defaultSearchTrigger,
}: TopBarProps) {
  const { vaultPath, switchVault, vaults } = useWorkspace();
  const openSettings = useSettingsStore((s) => s.openSettings);

  const activeFilePath = usePaneStore(selectActiveTabPath);
  const activeUnsaved = usePaneStore(selectActiveTabUnsaved);
  const { status: gitState } = useGitStatus(vaultPath);
  const branchLabel = gitState?.branch ?? "main";

  const workspace = vaultPath ? vaultPath.split("/").pop() ?? vaultPath : null;

  const handleWorkspaceClick =
    vaults.length > 1
      ? () => {
          // Quick-cycle: pick the next vault in the list.
          const idx = vaults.indexOf(vaultPath ?? "");
          const next = vaults[(idx + 1) % vaults.length];
          if (next && next !== vaultPath) void switchVault(next);
        }
      : undefined;

  return (
    <HStack
      data-tauri-drag-region
      data-atom="topbar"
      h="36px"
      pl="80px"
      pr={2}
      gap={3}
      bg="bg"
      borderBottomWidth="1px"
      borderColor="line"
      flexShrink={0}
    >
      <IconButton
        aria-label={sidebarOpen ? "Hide sidebar" : "Show sidebar"}
        variant="ghost"
        size="xs"
        onClick={onToggleSidebar}
      >
        <LuMenu />
      </IconButton>

      <Brand />

      <BreadcrumbNav
        workspace={workspace}
        filePath={activeFilePath}
        unsaved={activeUnsaved}
        onWorkspaceClick={handleWorkspaceClick}
      />

      <Box flex={1} />

      <SegmentedEnvSwitcher />

      <SearchTrigger
        type="button"
        data-atom="search-trigger"
        onClick={onSearch}
        aria-label="Search blocks, vars, schema"
        h="24px"
        w="220px"
        px={2}
        gap={2}
        display="inline-flex"
        alignItems="center"
        bg="bg.2"
        color="fg.3"
        borderWidth="1px"
        borderColor="line"
        borderRadius="4px"
        fontSize="11px"
        fontFamily="mono"
        cursor="pointer"
        _hover={{ bg: "bg.3", color: "fg.2" }}
      >
        <LuSearch size={12} />
        <Box flex={1} textAlign="left">
          Search blocks, vars, schema…
        </Box>
        <Kbd>⌘K</Kbd>
      </SearchTrigger>

      <Btn
        variant="ghost"
        data-atom="branch-btn"
        aria-label="Switch branch"
        h="24px"
        gap={2}
        title="Branch switcher (V10)"
      >
        <LuGitBranch size={12} />
        {branchLabel}
      </Btn>

      <Box w="1px" h="16px" bg="line" mx={1} aria-hidden />

      {onToggleOutlinePanel && (
        <IconButton
          aria-label={
            outlinePanelOpen ? "Close outline panel" : "Open outline panel"
          }
          variant="ghost"
          size="xs"
          onClick={onToggleOutlinePanel}
          color={outlinePanelOpen ? "accent" : undefined}
        >
          <LuListTree />
        </IconButton>
      )}
      {onToggleHistoryPanel && (
        <IconButton
          aria-label={
            historyPanelOpen ? "Close history panel" : "Open history panel"
          }
          variant="ghost"
          size="xs"
          onClick={onToggleHistoryPanel}
          color={historyPanelOpen ? "accent" : undefined}
        >
          <LuClock />
        </IconButton>
      )}
      <IconButton
        aria-label={schemaPanelOpen ? "Close schema panel" : "Open schema panel"}
        variant="ghost"
        size="xs"
        onClick={onToggleSchemaPanel}
        color={schemaPanelOpen ? "accent" : undefined}
      >
        <LuDatabase />
      </IconButton>
      <IconButton
        aria-label={chatOpen ? "Close chat" : "Open chat"}
        variant="ghost"
        size="xs"
        onClick={onToggleChat}
        color={chatOpen ? "accent" : undefined}
      >
        <LuMessageSquare />
      </IconButton>
      <IconButton
        aria-label="Settings"
        variant="ghost"
        size="xs"
        onClick={openSettings}
      >
        <LuSettings />
      </IconButton>
    </HStack>
  );
}
