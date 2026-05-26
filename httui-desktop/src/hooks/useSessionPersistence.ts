import { useEffect, useRef } from "react";
import {
  restoreSession,
  setConfig,
  startWatching,
  rebuildSearchIndex,
} from "@/lib/tauri/commands";
import { usePaneStore } from "@/stores/pane";
import { useWorkspaceStore } from "@/stores/workspace";
import { useSettingsStore } from "@/stores/settings";
import type { PaneLayout } from "@/types/pane";

// Remove tabs for files that no longer exist from the layout
function filterDeletedTabs(
  node: PaneLayout,
  existingFiles: Set<string>,
): PaneLayout {
  if (node.type === "leaf") {
    const validTabs = node.tabs.filter(
      (t) => t.kind !== "diff" && existingFiles.has(t.filePath),
    );
    return {
      ...node,
      tabs: validTabs,
      activeTab: Math.min(node.activeTab, Math.max(0, validTabs.length - 1)),
    };
  }
  return {
    ...node,
    children: [
      filterDeletedTabs(node.children[0], existingFiles),
      filterDeletedTabs(node.children[1], existingFiles),
    ],
  };
}

export function useSessionPersistence(): void {
  const sessionRestored = useRef(false);

  // Load session on startup — single IPC roundtrip
  useEffect(() => {
    (async () => {
      try {
        const session = await restoreSession();

        useWorkspaceStore.getState().setVaults(session.vaults);
        if (session.vim_enabled)
          useSettingsStore.getState().setVimEnabled(true);
        useSettingsStore.getState().setSidebarOpen(session.sidebar_open);

        if (session.active_vault) {
          useWorkspaceStore.getState().setVaultPath(session.active_vault);
          useWorkspaceStore.getState().setEntries(session.file_tree);

          // Fire-and-forget: start watching + rebuild index
          startWatching(session.active_vault).catch(() => {});
          rebuildSearchIndex(session.active_vault).catch(() => {});

          const { restoreLayout, scrollPositions } = usePaneStore.getState();

          if (session.pane_layout && session.active_pane_id) {
            try {
              const parsed = JSON.parse(session.pane_layout) as PaneLayout;

              const contents = new Map<string, string>();
              for (const tab of session.tab_contents) {
                if (tab.content) {
                  contents.set(tab.file_path, tab.content);
                }
              }

              const cleanedLayout = filterDeletedTabs(
                parsed,
                new Set(contents.keys()),
              );
              restoreLayout(cleanedLayout, session.active_pane_id, contents);

              // Restore scroll positions
              if (session.scroll_positions) {
                try {
                  const positions = JSON.parse(
                    session.scroll_positions,
                  ) as Record<string, number>;
                  const newPositions = new Map(scrollPositions);
                  for (const [fp, pos] of Object.entries(positions)) {
                    newPositions.set(fp, pos);
                  }
                  usePaneStore.setState({ scrollPositions: newPositions });
                } catch {
                  /* invalid JSON, ignore */
                }
              }
            } catch {
              // Invalid layout JSON, use default
            }
          } else if (session.active_file) {
            const tab = session.tab_contents[0];
            if (tab?.content) {
              const { openFile } = usePaneStore.getState();
              openFile(tab.file_path, tab.content, tab.vault_path);
            }
          }
        }
      } catch {
        // App may not be in Tauri context
      } finally {
        sessionRestored.current = true;
      }
    })();
  }, []);

  // Save pane layout on changes (only after session restore completes)
  useEffect(() => {
    return usePaneStore.subscribe((state, prevState) => {
      if (!sessionRestored.current) return;
      if (
        state.layout !== prevState.layout ||
        state.activePaneId !== prevState.activePaneId
      ) {
        setConfig("pane_layout", JSON.stringify(state.layout)).catch(() => {});
        setConfig("active_pane_id", state.activePaneId).catch(() => {});
        setConfig(
          "scroll_positions",
          JSON.stringify(Object.fromEntries(state.scrollPositions)),
        ).catch(() => {});
      }
    });
  }, []);

  // Save vim preference on changes (only after session restore completes)
  useEffect(() => {
    return useSettingsStore.subscribe((state, prevState) => {
      if (!sessionRestored.current) return;
      if (state.vimEnabled !== prevState.vimEnabled) {
        setConfig("vim_enabled", state.vimEnabled ? "true" : "false").catch(
          () => {},
        );
      }
      if (state.sidebarOpen !== prevState.sidebarOpen) {
        setConfig("sidebar_open", state.sidebarOpen ? "true" : "false").catch(
          () => {},
        );
      }
    });
  }, []);
}
