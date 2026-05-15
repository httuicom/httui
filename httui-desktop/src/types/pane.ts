export interface TabState {
  filePath: string;
  vaultPath: string;
  unsaved: boolean;
  kind?: "file" | "diff" | "connections" | "variables" | "environments" | "git";
  // Diff tab fields
  diffId?: string;
  permissionId?: string;
  originalContent?: string;
  proposedContent?: string;
}

/** Sentinel filePath for the singleton Connections tab (V4). The
 * pane store dedupes new opens by this value so only one
 * Connections tab can exist per pane. */
export const CONNECTIONS_TAB_PATH = "__connections__";

/** Sentinel filePath for the singleton Variables tab (V5). */
export const VARIABLES_TAB_PATH = "__variables__";

/** Sentinel filePath for the singleton Environments tab (V5). */
export const ENVIRONMENTS_TAB_PATH = "__environments__";

/** Sentinel filePath for the singleton Git panel tab (V10). */
export const GIT_TAB_PATH = "__git__";

export function getTabId(tab: TabState): string {
  return tab.diffId ?? tab.filePath;
}

export interface LeafPane {
  type: "leaf";
  id: string;
  tabs: TabState[];
  activeTab: number;
}

export interface SplitPane {
  type: "split";
  direction: "horizontal" | "vertical";
  children: [PaneLayout, PaneLayout];
  ratio: number; // 0-1, first child gets this fraction
}

export type PaneLayout = LeafPane | SplitPane;

let nextPaneId = 1;
export function createLeafPane(
  filePath?: string,
  vaultPath?: string,
): LeafPane {
  return {
    type: "leaf",
    id: `pane-${nextPaneId++}`,
    tabs: filePath
      ? [{ filePath, vaultPath: vaultPath ?? "", unsaved: false }]
      : [],
    activeTab: 0,
  };
}
