import { useEffect, useRef } from "react";
import { Box, Flex, Text } from "@chakra-ui/react";
import { TabBar } from "../TabBar";
import { DiffViewer } from "@/components/editor/DiffViewer";
import { ConnectionsPageContainer } from "@/components/layout/connections/ConnectionsPageContainer";
import { GitPanelContainer } from "@/components/layout/git/GitPanelContainer";
import { EnvironmentsPageContainer } from "@/components/layout/environments/EnvironmentsPageContainer";
import { VariablesPageContainer } from "@/components/layout/variables/VariablesPageContainer";
import { DocHeaderedEditor } from "./DocHeaderedEditor";
import { usePaneStore } from "@/stores/pane";
import { useSettingsStore } from "@/stores/settings";
import { SplitView } from "./SplitView";
import type { PaneLayout } from "@/types/pane";
import { readNote } from "@/lib/tauri/commands";

interface PaneNodeProps {
  layout: PaneLayout;
  path: number[];
  handleEditorChange: (
    paneId: string,
    filePath: string,
    content: string,
    vaultPath: string,
  ) => void;
  onNavigateFile?: (filePath: string) => void;
}

export function PaneNode({
  layout,
  path,
  handleEditorChange,
  onNavigateFile,
}: PaneNodeProps) {
  const editorContents = usePaneStore((s) => s.editorContents);
  const unsavedFiles = usePaneStore((s) => s.unsavedFiles);
  const hasConflict = usePaneStore((s) => s.hasConflict);
  const resolveConflict = usePaneStore((s) => s.resolveConflict);
  const openFile = usePaneStore((s) => s.openFile);
  const setActivePaneId = usePaneStore((s) => s.setActivePaneId);
  const selectTab = usePaneStore((s) => s.selectTab);
  const closeTab = usePaneStore((s) => s.closeTab);
  const closeOthers = usePaneStore((s) => s.closeOthers);
  const closeAll = usePaneStore((s) => s.closeAll);
  const vimEnabled = useSettingsStore((s) => s.vimEnabled);
  // navigateFile passed via prop chain — WorkspaceContext subscription would
  // re-render on every file-tree change, resetting CM6 scroll position.

  // Re-read files cached as HTML by the legacy TipTap editor on first open.
  // Detected by a leading `<` — markdown never starts with an HTML tag.
  const recoveredRef = useRef<Set<string>>(new Set());
  useEffect(() => {
    if (layout.type !== "leaf") return;
    for (const tab of layout.tabs) {
      if (
        tab.kind === "diff" ||
        tab.kind === "connections" ||
        tab.kind === "variables" ||
        tab.kind === "environments" ||
        tab.kind === "git"
      )
        continue;
      const cached = editorContents.get(tab.filePath);
      if (
        cached &&
        cached.trimStart().startsWith("<") &&
        !recoveredRef.current.has(tab.filePath)
      ) {
        recoveredRef.current.add(tab.filePath);
        readNote(tab.vaultPath, tab.filePath)
          .then((md) => openFile(tab.filePath, md, tab.vaultPath))
          .catch(() => {});
      }
    }
  }, [layout, editorContents, openFile]);

  if (layout.type === "leaf") {
    const activeTab = layout.tabs[layout.activeTab];
    const content = activeTab
      ? (editorContents.get(activeTab.filePath) ?? "")
      : "";

    return (
      <Flex
        direction="column"
        flex={1}
        overflow="hidden"
        onClick={() => setActivePaneId(layout.id)}
      >
        <TabBar
          tabs={layout.tabs}
          activeTab={layout.activeTab}
          unsavedFiles={unsavedFiles}
          onSelectTab={(index) => selectTab(layout.id, index)}
          onCloseTab={(index) => closeTab(layout.id, index)}
          onCloseOthers={(index) => closeOthers(layout.id, index)}
          onCloseAll={() => closeAll(layout.id)}
        />
        {activeTab ? (
          activeTab.kind === "diff" ? (
            <Box flex={1} overflow="hidden">
              <DiffViewer tab={activeTab} />
            </Box>
          ) : activeTab.kind === "connections" ? (
            <Box flex={1} overflow="hidden">
              <ConnectionsPageContainer onNavigateFile={onNavigateFile} />
            </Box>
          ) : activeTab.kind === "variables" ? (
            <Box flex={1} overflow="hidden">
              <VariablesPageContainer onNavigateFile={onNavigateFile} />
            </Box>
          ) : activeTab.kind === "environments" ? (
            <Box flex={1} overflow="hidden">
              <EnvironmentsPageContainer />
            </Box>
          ) : activeTab.kind === "git" ? (
            <Box flex={1} overflow="hidden">
              <GitPanelContainer onNavigateFile={onNavigateFile} />
            </Box>
          ) : (
            <DocHeaderedEditor
              filePath={activeTab.filePath}
              vaultPath={activeTab.vaultPath}
              content={content}
              vimEnabled={vimEnabled}
              showConflict={hasConflict(activeTab.filePath)}
              dirty={unsavedFiles.has(activeTab.filePath)}
              onConflictReload={() =>
                resolveConflict(
                  activeTab.filePath,
                  "reload",
                  activeTab.vaultPath,
                )
              }
              onConflictKeep={() =>
                resolveConflict(activeTab.filePath, "keep", null)
              }
              onChange={(c) =>
                handleEditorChange(
                  layout.id,
                  activeTab.filePath,
                  c,
                  activeTab.vaultPath,
                )
              }
              onNavigateFile={onNavigateFile}
            />
          )
        ) : (
          <Flex flex={1} align="center" justify="center">
            <Text fontSize="sm" color="fg.muted">
              Open a file to start editing
            </Text>
          </Flex>
        )}
      </Flex>
    );
  }

  return (
    <SplitView
      layout={layout}
      path={path}
      handleEditorChange={handleEditorChange}
      onNavigateFile={onNavigateFile}
    />
  );
}
