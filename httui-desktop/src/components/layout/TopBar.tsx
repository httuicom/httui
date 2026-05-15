// Workbench top bar — canvas §4 (36px tall).
//
// Layout:
//   [Sidebar toggle][Brand][Breadcrumb] ━━━
//   [SearchPlaceholder ⌘K] [Schema][Chat][Settings]
//
// The Sidebar/Chat/Schema/Settings toggles are functional necessities
// and live on the right edge. Brand wordmark is centered with the
// Tauri drag region in mind (`pl="80px"` reserves space for macOS
// traffic-light buttons).
//
// Search ⌘K re-dispatches Cmd+P so the existing keyboard route picks
// it up. The previous Run-all button (dropped 2026-05-01), env
// switcher (moved to StatusBar `EnvMenu`) and branch button (moved
// to StatusBar `BranchMenu`) are no longer in the topbar — V2 keeps
// the chrome to the breadcrumb + search + the right-edge toggles.

import { Box, HStack, IconButton, chakra } from "@chakra-ui/react";
import {
  LuMenu,
  LuSearch,
  LuMessageSquare,
  LuSettings,
  LuDatabase,
  LuListTree,
  LuClock,
  LuPlug,
  LuKeyRound,
  LuLayers,
  LuGitBranch,
} from "react-icons/lu";

import { Brand } from "@/components/layout/topbar/Brand";
import { BreadcrumbNav } from "@/components/layout/topbar/BreadcrumbNav";
import { WorkspaceMenu } from "@/components/layout/topbar/WorkspaceMenu";
import { Kbd } from "@/components/atoms";
import { useWorkspace } from "@/contexts/WorkspaceContext";
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
  const { vaultPath, switchVault, vaults, openVault } = useWorkspace();
  const openSettings = useSettingsStore((s) => s.openSettings);
  const openConnectionsTab = usePaneStore((s) => s.openConnectionsTab);
  const openVariablesTab = usePaneStore((s) => s.openVariablesTab);
  const openEnvironmentsTab = usePaneStore((s) => s.openEnvironmentsTab);
  const openGitTab = usePaneStore((s) => s.openGitTab);

  const activeFilePath = usePaneStore(selectActiveTabPath);
  const activeUnsaved = usePaneStore(selectActiveTabUnsaved);

  const workspace = vaultPath ? vaultPath.split("/").pop() ?? vaultPath : null;
  const isLeafSegment = !activeFilePath;

  const workspaceSlot = workspace ? (
    <WorkspaceMenu
      workspace={workspace}
      isLeaf={isLeafSegment}
      vaults={vaults}
      activeVault={vaultPath}
      onSwitch={(path) => {
        if (path !== vaultPath) void switchVault(path);
      }}
      onOpenOther={() => void openVault()}
    />
  ) : undefined;

  return (
    <HStack
      data-tauri-drag-region
      data-atom="topbar"
      h="36px"
      minH="36px"
      maxH="36px"
      pl="80px"
      pr={2}
      gap={3}
      bg="bg"
      borderBottomWidth="1px"
      borderColor="border"
      flexShrink={0}
      overflow="hidden"
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
        workspaceSlot={workspaceSlot}
      />

      <Box flex={1} />

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
        bg="bg.muted"
        color="fg.subtle"
        borderWidth="1px"
        borderColor="border"
        borderRadius="4px"
        fontSize="11px"
        fontFamily="mono"
        cursor="pointer"
        whiteSpace="nowrap"
        overflow="hidden"
        flexShrink={0}
        _hover={{ bg: "bg.emphasized", color: "fg.muted" }}
      >
        <LuSearch size={12} style={{ flexShrink: 0 }} />
        <Box
          flex={1}
          minW={0}
          textAlign="left"
          overflow="hidden"
          textOverflow="ellipsis"
          whiteSpace="nowrap"
        >
          Search blocks, vars, schema…
        </Box>
        <Kbd>⌘K</Kbd>
      </SearchTrigger>

      <Box w="1px" h="16px" bg="border" mx={1} aria-hidden />

      {onToggleOutlinePanel && (
        <IconButton
          aria-label={
            outlinePanelOpen ? "Close outline panel" : "Open outline panel"
          }
          variant="ghost"
          size="xs"
          onClick={onToggleOutlinePanel}
          color={outlinePanelOpen ? "brand.fg" : undefined}
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
          color={historyPanelOpen ? "brand.fg" : undefined}
        >
          <LuClock />
        </IconButton>
      )}
      <IconButton
        aria-label="Open Connections"
        variant="ghost"
        size="xs"
        onClick={openConnectionsTab}
      >
        <LuPlug />
      </IconButton>
      <IconButton
        aria-label="Open Variables"
        variant="ghost"
        size="xs"
        onClick={openVariablesTab}
      >
        <LuKeyRound />
      </IconButton>
      <IconButton
        aria-label="Open Environments"
        variant="ghost"
        size="xs"
        onClick={openEnvironmentsTab}
      >
        <LuLayers />
      </IconButton>
      <IconButton
        aria-label="Open Git"
        variant="ghost"
        size="xs"
        onClick={openGitTab}
      >
        <LuGitBranch />
      </IconButton>
      <IconButton
        aria-label={schemaPanelOpen ? "Close schema panel" : "Open schema panel"}
        variant="ghost"
        size="xs"
        onClick={onToggleSchemaPanel}
        color={schemaPanelOpen ? "brand.fg" : undefined}
      >
        <LuDatabase />
      </IconButton>
      <IconButton
        aria-label={chatOpen ? "Close chat" : "Open chat"}
        variant="ghost"
        size="xs"
        onClick={onToggleChat}
        color={chatOpen ? "brand.fg" : undefined}
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
