import { create } from "zustand";
import { devtools } from "zustand/middleware";
import { useShallow } from "zustand/react/shallow";
import { listen } from "@tauri-apps/api/event";
import type { PaneLayout, LeafPane, TabState } from "@/types/pane";
import {
  createLeafPane,
  CONNECTIONS_TAB_PATH,
  VARIABLES_TAB_PATH,
  ENVIRONMENTS_TAB_PATH,
  GIT_TAB_PATH,
} from "@/types/pane";
import { forceReloadFile } from "@/lib/tauri/commands";

// --- Types ---

export interface DiffTabParams {
  filePath: string;
  vaultPath: string;
  permissionId: string;
  originalContent: string;
  proposedContent: string;
}

interface FileReloadedPayload {
  path: string;
  markdown: string;
}

interface PaneState {
  // State
  layout: PaneLayout;
  activePaneId: string;
  editorContents: Map<string, string>;
  unsavedFiles: Set<string>;
  scrollPositions: Map<string, number>;
  conflictFiles: Set<string>;
  /** Monotonic counter incremented after every successful auto-save.
   *  Reactive — consumers that need to react to "a save just landed"
   *  subscribe to this instead of `unsavedFiles` (which is mutated
   *  in-place for keystroke perf and therefore doesn't notify React).
   *  Bumped by `notifySaved`. */
  saveSignal: number;

  // Computed
  getActiveLeaf: () => LeafPane | null;

  // Pane actions
  openFile: (filePath: string, content: string, vaultPath: string) => void;
  openDiffTab: (params: DiffTabParams) => void;
  closeDiffTab: (permissionId: string) => void;
  /** Open the singleton Connections tab in the active pane (V4).
   * If a Connections tab already exists in the pane, focuses it
   * instead of opening a duplicate. */
  openConnectionsTab: () => void;
  /** Open the singleton Variables tab in the active pane (V5). */
  openVariablesTab: () => void;
  /** Open the singleton Environments tab in the active pane (V5). */
  openEnvironmentsTab: () => void;
  /** Open the singleton Git panel tab in the active pane (V10). */
  openGitTab: () => void;
  selectTab: (paneId: string, index: number) => void;
  closeTab: (paneId: string, index: number) => void;
  closeOthers: (paneId: string, index: number) => void;
  closeAll: (paneId: string) => void;
  setActivePaneId: (paneId: string) => void;
  splitVertical: () => void;
  splitHorizontal: () => void;
  nextTab: () => void;
  updateContent: (filePath: string, content: string) => void;
  markUnsaved: (paneId: string, filePath: string, unsaved: boolean) => void;
  /** Increment `saveSignal`. Called by the auto-save flow once
   *  `writeNote` resolves. */
  notifySaved: () => void;
  resizeSplit: (path: number[], ratio: number) => void;
  restoreLayout: (
    layout: PaneLayout,
    activePaneId: string,
    contents?: Map<string, string>,
  ) => void;
  setScrollPosition: (filePath: string, position: number) => void;
  getScrollPosition: (filePath: string) => number | undefined;

  // Conflict actions
  hasConflict: (filePath: string) => boolean;
  resolveConflict: (
    filePath: string,
    action: "reload" | "keep",
    vaultPath: string | null,
  ) => Promise<void>;
}

// --- Pure helper functions (exported for testing) ---

export function findLeaf(node: PaneLayout, id: string): LeafPane | null {
  if (node.type === "leaf") return node.id === id ? node : null;
  return findLeaf(node.children[0], id) ?? findLeaf(node.children[1], id);
}

export function updateLeaf(
  node: PaneLayout,
  id: string,
  updater: (leaf: LeafPane) => LeafPane,
): PaneLayout {
  if (node.type === "leaf") return node.id === id ? updater({ ...node }) : node;
  return {
    ...node,
    children: [
      updateLeaf(node.children[0], id, updater),
      updateLeaf(node.children[1], id, updater),
    ],
  };
}

export function removeLeaf(node: PaneLayout, id: string): PaneLayout | null {
  if (node.type === "leaf") return node.id === id ? null : node;
  const left = removeLeaf(node.children[0], id);
  const right = removeLeaf(node.children[1], id);
  if (!left) return right;
  if (!right) return left;
  return { ...node, children: [left, right] };
}

export function allLeafIds(node: PaneLayout): string[] {
  if (node.type === "leaf") return [node.id];
  return [...allLeafIds(node.children[0]), ...allLeafIds(node.children[1])];
}

export function updateSplitRatio(
  node: PaneLayout,
  path: number[],
  ratio: number,
): PaneLayout {
  if (path.length === 0 && node.type === "split") return { ...node, ratio };
  if (node.type === "split" && path.length > 0) {
    const [head, ...rest] = path;
    const children: [PaneLayout, PaneLayout] = [...node.children];
    children[head] = updateSplitRatio(children[head], rest, ratio);
    return { ...node, children };
  }
  return node;
}

export function replacePaneInLayout(
  node: PaneLayout,
  id: string,
  replacement: PaneLayout,
): PaneLayout {
  if (node.type === "leaf") return node.id === id ? replacement : node;
  return {
    ...node,
    children: [
      replacePaneInLayout(node.children[0], id, replacement),
      replacePaneInLayout(node.children[1], id, replacement),
    ],
  };
}

// --- Singleton tab helper ---

type SingletonTabKind = "connections" | "variables" | "environments" | "git";

function openSingletonTab(
  set: (fn: (state: PaneState) => PaneState | Partial<PaneState>) => void,
  get: () => PaneState,
  kind: SingletonTabKind,
  filePath: string,
): void {
  const { activePaneId } = get();
  set((state) => {
    const leaf = findLeaf(state.layout, activePaneId);
    if (!leaf) return state;
    // Singleton: focus existing tab of same kind in this pane instead
    // of duplicating. Identity = (pane, kind); the sentinel filePath
    // also serves as the editor-content map key.
    const existing = leaf.tabs.findIndex((t) => t.kind === kind);
    if (existing >= 0) {
      return {
        layout: updateLeaf(state.layout, activePaneId, (l) => ({
          ...l,
          activeTab: existing,
        })),
      };
    }
    const tab: TabState = {
      filePath,
      vaultPath: "",
      unsaved: false,
      kind,
    };
    return {
      layout: updateLeaf(state.layout, activePaneId, (l) => ({
        ...l,
        tabs: [...l.tabs, tab],
        activeTab: l.tabs.length,
      })),
    };
  });
}

// --- Store ---

const initialLeaf = createLeafPane();

export const usePaneStore = create<PaneState>()(
  devtools(
    (set, get) => ({
      // Initial state
      layout: initialLeaf,
      activePaneId: initialLeaf.id,
      editorContents: new Map<string, string>(),
      unsavedFiles: new Set<string>(),
      scrollPositions: new Map<string, number>(),
      conflictFiles: new Set<string>(),
      saveSignal: 0,

      // Computed
      getActiveLeaf: () => findLeaf(get().layout, get().activePaneId),

      // --- Pane actions ---

      openFile: (filePath, content, vaultPath) => {
        const { activePaneId } = get();
        // Mutate content in place (non-reactive, like updateContent)
        get().editorContents.set(filePath, content);

        set((state) => {
          const leaf = findLeaf(state.layout, activePaneId);
          if (!leaf) return state;

          const existing = leaf.tabs.findIndex((t) => t.filePath === filePath);
          if (existing >= 0) {
            return {
              layout: updateLeaf(state.layout, activePaneId, (l) => ({
                ...l,
                activeTab: existing,
              })),
            };
          }
          return {
            layout: updateLeaf(state.layout, activePaneId, (l) => ({
              ...l,
              tabs: [...l.tabs, { filePath, vaultPath, unsaved: false }],
              activeTab: l.tabs.length,
            })),
          };
        });
      },

      selectTab: (paneId, index) => {
        set((state) => ({
          layout: updateLeaf(state.layout, paneId, (l) => ({
            ...l,
            activeTab: index,
          })),
          activePaneId: paneId,
        }));
      },

      closeTab: (paneId, index) => {
        set((state) => {
          const leaf = findLeaf(state.layout, paneId);
          if (!leaf) return state;

          const newTabs = leaf.tabs.filter((_, i) => i !== index);
          if (newTabs.length === 0) {
            const result = removeLeaf(state.layout, paneId);
            if (result) {
              const ids = allLeafIds(result);
              return {
                layout: result,
                activePaneId: ids.length > 0 ? ids[0] : state.activePaneId,
              };
            }
            return {
              layout: updateLeaf(state.layout, paneId, (l) => ({
                ...l,
                tabs: [],
                activeTab: 0,
              })),
            };
          }
          const newActive = Math.min(leaf.activeTab, newTabs.length - 1);
          return {
            layout: updateLeaf(state.layout, paneId, (l) => ({
              ...l,
              tabs: newTabs,
              activeTab: newActive,
            })),
          };
        });
      },

      closeOthers: (paneId, index) => {
        set((state) => ({
          layout: updateLeaf(state.layout, paneId, (l) => ({
            ...l,
            tabs: [l.tabs[index]],
            activeTab: 0,
          })),
        }));
      },

      closeAll: (paneId) => {
        set((state) => ({
          layout: updateLeaf(state.layout, paneId, (l) => ({
            ...l,
            tabs: [],
            activeTab: 0,
          })),
        }));
      },

      setActivePaneId: (paneId) => {
        set({ activePaneId: paneId });
      },

      splitVertical: () => {
        const { activePaneId } = get();
        const newPane = createLeafPane();
        set((state) => {
          const leaf = findLeaf(state.layout, activePaneId);
          if (!leaf) return state;
          return {
            layout: replacePaneInLayout(state.layout, activePaneId, {
              type: "split",
              direction: "vertical",
              children: [leaf, newPane],
              ratio: 0.5,
            }),
            activePaneId: newPane.id,
          };
        });
      },

      splitHorizontal: () => {
        const { activePaneId } = get();
        const newPane = createLeafPane();
        set((state) => {
          const leaf = findLeaf(state.layout, activePaneId);
          if (!leaf) return state;
          return {
            layout: replacePaneInLayout(state.layout, activePaneId, {
              type: "split",
              direction: "horizontal",
              children: [leaf, newPane],
              ratio: 0.5,
            }),
            activePaneId: newPane.id,
          };
        });
      },

      nextTab: () => {
        const { activePaneId } = get();
        set((state) => {
          const leaf = findLeaf(state.layout, activePaneId);
          if (!leaf || leaf.tabs.length <= 1) return state;
          const next = (leaf.activeTab + 1) % leaf.tabs.length;
          return {
            layout: updateLeaf(state.layout, activePaneId, (l) => ({
              ...l,
              activeTab: next,
            })),
          };
        });
      },

      updateContent: (filePath, content) => {
        // Mutate in place — intentionally non-reactive.
        // Content changes happen on every keystroke; triggering re-renders
        // would cause editors to remount and lose scroll position.
        get().editorContents.set(filePath, content);
      },

      markUnsaved: (_paneId, filePath, unsaved) => {
        // Mutate in place — intentionally non-reactive.
        // Same reason as updateContent: called on every keystroke.
        if (unsaved) {
          get().unsavedFiles.add(filePath);
        } else {
          get().unsavedFiles.delete(filePath);
        }
      },

      notifySaved: () => {
        set((s) => ({ saveSignal: s.saveSignal + 1 }));
      },

      resizeSplit: (path, ratio) => {
        set((state) => ({
          layout: updateSplitRatio(state.layout, path, ratio),
        }));
      },

      setScrollPosition: (filePath, position) => {
        // Mutate in place for performance (called on every scroll event).
        // Components don't subscribe to scroll positions reactively.
        get().scrollPositions.set(filePath, position);
      },

      getScrollPosition: (filePath) => {
        return get().scrollPositions.get(filePath);
      },

      openDiffTab: (params) => {
        const { activePaneId } = get();
        const diffId = `diff-${params.permissionId}`;
        set((state) => {
          const leaf = findLeaf(state.layout, activePaneId);
          if (!leaf) return state;

          const existing = leaf.tabs.findIndex((t) => t.diffId === diffId);
          if (existing >= 0) {
            return {
              layout: updateLeaf(state.layout, activePaneId, (l) => ({
                ...l,
                activeTab: existing,
              })),
            };
          }
          const tab: TabState = {
            filePath: params.filePath,
            vaultPath: params.vaultPath,
            unsaved: false,
            kind: "diff",
            diffId,
            permissionId: params.permissionId,
            originalContent: params.originalContent,
            proposedContent: params.proposedContent,
          };
          return {
            layout: updateLeaf(state.layout, activePaneId, (l) => ({
              ...l,
              tabs: [...l.tabs, tab],
              activeTab: l.tabs.length,
            })),
          };
        });
      },

      openConnectionsTab: () =>
        openSingletonTab(set, get, "connections", CONNECTIONS_TAB_PATH),
      openVariablesTab: () =>
        openSingletonTab(set, get, "variables", VARIABLES_TAB_PATH),
      openEnvironmentsTab: () =>
        openSingletonTab(set, get, "environments", ENVIRONMENTS_TAB_PATH),
      openGitTab: () => openSingletonTab(set, get, "git", GIT_TAB_PATH),

      closeDiffTab: (permissionId) => {
        const diffId = `diff-${permissionId}`;
        set((state) => {
          const leaves = allLeafIds(state.layout);
          for (const leafId of leaves) {
            const leaf = findLeaf(state.layout, leafId);
            if (!leaf) continue;
            const idx = leaf.tabs.findIndex((t) => t.diffId === diffId);
            if (idx >= 0) {
              const newTabs = leaf.tabs.filter((_, i) => i !== idx);
              if (newTabs.length === 0) {
                const result = removeLeaf(state.layout, leafId);
                if (result) return { layout: result };
                return {
                  layout: updateLeaf(state.layout, leafId, (l) => ({
                    ...l,
                    tabs: [],
                    activeTab: 0,
                  })),
                };
              }
              const newActive = Math.min(leaf.activeTab, newTabs.length - 1);
              return {
                layout: updateLeaf(state.layout, leafId, (l) => ({
                  ...l,
                  tabs: newTabs,
                  activeTab: newActive,
                })),
              };
            }
          }
          return state;
        });
      },

      restoreLayout: (savedLayout, savedActivePaneId, contents) => {
        // Mutate content map in place (non-reactive)
        if (contents) {
          const editorContents = get().editorContents;
          for (const [filePath, html] of contents) {
            editorContents.set(filePath, html);
          }
        }
        set({ layout: savedLayout, activePaneId: savedActivePaneId });
      },

      // --- Conflict actions ---

      hasConflict: (filePath) => get().conflictFiles.has(filePath),

      resolveConflict: async (filePath, action, vaultPath) => {
        if (action === "reload" && vaultPath) {
          try {
            await forceReloadFile(vaultPath, filePath);
          } catch (err) {
            console.error("Failed to reload file:", err);
          }
        }
        set((state) => {
          const next = new Set(state.conflictFiles);
          next.delete(filePath);
          return { conflictFiles: next };
        });
      },
    }),
    { name: "pane-store" },
  ),
);

// --- Tauri event listeners ---

export function setupPaneListeners() {
  listen<FileReloadedPayload>("file-reloaded", (event) => {
    const { path } = event.payload;
    const { editorContents, unsavedFiles, conflictFiles } =
      usePaneStore.getState();

    // Only care about open files
    if (!editorContents.has(path)) return;

    // If file has unsaved edits, show conflict banner
    if (unsavedFiles.has(path)) {
      usePaneStore.setState({
        conflictFiles: new Set(conflictFiles).add(path),
      });
    }
  });
}

// --- Selectors ---

export const selectLayout = (s: PaneState) => s.layout;
export const selectActivePaneId = (s: PaneState) => s.activePaneId;
export const selectEditorContents = (s: PaneState) => s.editorContents;
export const selectUnsavedFiles = (s: PaneState) => s.unsavedFiles;
export const selectScrollPositions = (s: PaneState) => s.scrollPositions;

export function useLayoutAndActive() {
  return usePaneStore(
    useShallow((s) => ({ layout: s.layout, activePaneId: s.activePaneId })),
  );
}

/**
 * Walk the pane tree to find the active tab on the active pane and
 * return its `filePath`. Used by the TopBar breadcrumb to surface
 * the currently-focused file. Returns `null` when no leaf has any
 * tabs (cold start / empty vault).
 */
export function selectActiveTabPath(s: PaneState): string | null {
  function find(
    node: PaneLayout,
  ): { filePath: string; unsaved: boolean } | null {
    if (node.type === "split") {
      return find(node.children[0]) ?? find(node.children[1]);
    }
    if (s.activePaneId && node.id === s.activePaneId) {
      const tab = node.tabs[node.activeTab];
      return tab ? { filePath: tab.filePath, unsaved: tab.unsaved } : null;
    }
    return null;
  }
  // First try exact match on activePaneId.
  const direct = find(s.layout);
  if (direct) return direct.filePath;
  // Fall back to the first leaf with a tab — handles "no active pane
  // marked" cold-start.
  function firstWithTab(node: PaneLayout): string | null {
    if (node.type === "split") {
      return firstWithTab(node.children[0]) ?? firstWithTab(node.children[1]);
    }
    return node.tabs[node.activeTab]?.filePath ?? null;
  }
  return firstWithTab(s.layout);
}

/**
 * True when the *active* tab on the active pane has unsaved changes.
 * Powers the dirty-dot indicator on the breadcrumb's last segment.
 */
export function selectActiveTabUnsaved(s: PaneState): boolean {
  const path = selectActiveTabPath(s);
  if (!path) return false;
  return s.unsavedFiles.has(path);
}
