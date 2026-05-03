import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import {
  usePaneStore,
  setupPaneListeners,
  findLeaf,
  updateLeaf,
  removeLeaf,
  allLeafIds,
  updateSplitRatio,
  replacePaneInLayout,
  selectActiveTabPath,
  selectActiveTabUnsaved,
} from "@/stores/pane";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";
import {
  emitTauriEvent,
  clearTauriListeners,
  listen,
} from "@/test/mocks/tauri-event";
import type { PaneLayout, LeafPane, SplitPane } from "@/types/pane";

const VAULT = "/v";

const mkLeaf = (id: string, tabs: LeafPane["tabs"] = []): LeafPane => ({
  type: "leaf",
  id,
  tabs,
  activeTab: 0,
});

const mkSplit = (
  children: [PaneLayout, PaneLayout],
  direction: "horizontal" | "vertical" = "vertical",
): SplitPane => ({
  type: "split",
  direction,
  children,
  ratio: 0.5,
});

function resetStore() {
  usePaneStore.setState({
    layout: { type: "leaf", id: "p1", tabs: [], activeTab: 0 },
    activePaneId: "p1",
    editorContents: new Map(),
    unsavedFiles: new Set(),
    scrollPositions: new Map(),
    conflictFiles: new Set(),
  });
}

describe("paneStore — extended coverage", () => {
  beforeEach(() => {
    resetStore();
    clearTauriMocks();
    clearTauriListeners();
    listen.mockClear();
  });

  afterEach(() => {
    clearTauriMocks();
    clearTauriListeners();
  });

  // ──────────────────────────────────────────────
  // Pure helpers
  // ──────────────────────────────────────────────
  describe("pure helpers", () => {
    it("findLeaf returns null for nonexistent id", () => {
      expect(findLeaf(mkLeaf("a"), "b")).toBeNull();
    });

    it("findLeaf descends into splits", () => {
      const tree = mkSplit([mkLeaf("l"), mkLeaf("r")]);
      expect(findLeaf(tree, "r")?.id).toBe("r");
    });

    it("updateLeaf is identity for unmatched id", () => {
      const leaf = mkLeaf("a");
      const out = updateLeaf(leaf, "b", (l) => ({ ...l, activeTab: 99 }));
      expect(out).toBe(leaf);
    });

    it("removeLeaf collapses split when one child is removed", () => {
      const tree = mkSplit([mkLeaf("l"), mkLeaf("r")]);
      const out = removeLeaf(tree, "l");
      expect(out?.type).toBe("leaf");
      if (out?.type === "leaf") expect(out.id).toBe("r");
    });

    it("removeLeaf returns null when removing the only leaf", () => {
      expect(removeLeaf(mkLeaf("solo"), "solo")).toBeNull();
    });

    it("removeLeaf preserves tree when removing nothing", () => {
      const tree = mkSplit([mkLeaf("a"), mkLeaf("b")]);
      const out = removeLeaf(tree, "missing");
      expect(out?.type).toBe("split");
    });

    it("allLeafIds returns ids in DFS order", () => {
      const tree = mkSplit([mkSplit([mkLeaf("a"), mkLeaf("b")]), mkLeaf("c")]);
      expect(allLeafIds(tree)).toEqual(["a", "b", "c"]);
    });

    it("updateSplitRatio walks the path correctly", () => {
      const tree = mkSplit([mkSplit([mkLeaf("a"), mkLeaf("b")]), mkLeaf("c")]);
      const out = updateSplitRatio(tree, [0], 0.7) as SplitPane;
      const child = out.children[0] as SplitPane;
      expect(child.ratio).toBe(0.7);
    });

    it("updateSplitRatio is identity when path doesn't reach a split", () => {
      const tree = mkLeaf("solo");
      expect(updateSplitRatio(tree, [0], 0.7)).toBe(tree);
    });

    it("replacePaneInLayout swaps the matched leaf", () => {
      const tree = mkSplit([mkLeaf("a"), mkLeaf("b")]);
      const replacement = mkLeaf("a-prime");
      const out = replacePaneInLayout(tree, "a", replacement) as SplitPane;
      expect((out.children[0] as LeafPane).id).toBe("a-prime");
    });
  });

  // ──────────────────────────────────────────────
  // Diff tabs
  // ──────────────────────────────────────────────
  describe("diff tabs", () => {
    const params = {
      filePath: "x.md",
      vaultPath: VAULT,
      permissionId: "perm-1",
      originalContent: "old",
      proposedContent: "new",
    };

    it("openDiffTab adds a diff tab to the active leaf", () => {
      usePaneStore.getState().openDiffTab(params);
      const layout = usePaneStore.getState().layout;
      if (layout.type === "leaf") {
        expect(layout.tabs).toHaveLength(1);
        expect(layout.tabs[0].kind).toBe("diff");
        expect(layout.tabs[0].diffId).toBe("diff-perm-1");
      }
    });

    it("openDiffTab focuses existing diff instead of duplicating", () => {
      usePaneStore.getState().openDiffTab(params);
      // Open another to push diff away
      usePaneStore.getState().openFile("other.md", "x", VAULT);
      usePaneStore.getState().openDiffTab(params);

      const layout = usePaneStore.getState().layout;
      if (layout.type === "leaf") {
        expect(layout.tabs).toHaveLength(2);
        expect(layout.tabs.filter((t) => t.kind === "diff")).toHaveLength(1);
        expect(layout.activeTab).toBe(0); // diff was at index 0
      }
    });

    it("closeDiffTab removes the diff tab by permissionId", () => {
      usePaneStore.getState().openDiffTab(params);
      usePaneStore.getState().closeDiffTab("perm-1");
      const layout = usePaneStore.getState().layout;
      if (layout.type === "leaf") {
        expect(layout.tabs).toHaveLength(0);
      }
    });

    it("closeDiffTab is no-op for unknown permissionId", () => {
      usePaneStore.getState().openDiffTab(params);
      usePaneStore.getState().closeDiffTab("ghost");
      const layout = usePaneStore.getState().layout;
      if (layout.type === "leaf") {
        expect(layout.tabs).toHaveLength(1);
      }
    });
  });

  // ──────────────────────────────────────────────
  // Connections tab (V4)
  // ──────────────────────────────────────────────
  describe("connections tab", () => {
    it("openConnectionsTab adds a singleton tab to the active pane", () => {
      usePaneStore.getState().openConnectionsTab();
      const layout = usePaneStore.getState().layout;
      if (layout.type === "leaf") {
        expect(layout.tabs).toHaveLength(1);
        expect(layout.tabs[0].kind).toBe("connections");
        expect(layout.tabs[0].filePath).toBe("__connections__");
      }
    });

    it("opening twice focuses the existing tab instead of duplicating", () => {
      usePaneStore.getState().openConnectionsTab();
      usePaneStore.getState().openFile("other.md", "x", VAULT);
      usePaneStore.getState().openConnectionsTab();

      const layout = usePaneStore.getState().layout;
      if (layout.type === "leaf") {
        expect(layout.tabs).toHaveLength(2);
        expect(
          layout.tabs.filter((t) => t.kind === "connections"),
        ).toHaveLength(1);
        expect(layout.activeTab).toBe(0);
      }
    });

    it("singleton survives closing other tabs without duplication", () => {
      usePaneStore.getState().openConnectionsTab();
      usePaneStore.getState().openFile("other.md", "x", VAULT);
      const paneId = usePaneStore.getState().activePaneId;
      usePaneStore.getState().closeTab(paneId, 1);
      usePaneStore.getState().openConnectionsTab();

      const layout = usePaneStore.getState().layout;
      if (layout.type === "leaf") {
        expect(
          layout.tabs.filter((t) => t.kind === "connections"),
        ).toHaveLength(1);
      }
    });
  });

  // ──────────────────────────────────────────────
  // Splits + tab close edge cases
  // ──────────────────────────────────────────────
  describe("split / close edge cases", () => {
    it("closeTab on last tab in a split pane removes the pane and collapses", () => {
      // Start: leaf p1 with one tab
      usePaneStore.getState().openFile("a.md", "x", VAULT);
      // Split → creates p2 (active)
      usePaneStore.getState().splitVertical();
      const splitActive = usePaneStore.getState().activePaneId;

      // Add a tab on the new pane and close it
      usePaneStore.getState().openFile("b.md", "y", VAULT);
      usePaneStore.getState().closeTab(splitActive, 0);

      // Layout should collapse back to a single leaf
      const layout = usePaneStore.getState().layout;
      expect(layout.type).toBe("leaf");
    });

    it("selectTab updates activeTab and activePaneId", () => {
      usePaneStore.getState().openFile("a.md", "x", VAULT);
      usePaneStore.getState().openFile("b.md", "y", VAULT);

      const paneId = usePaneStore.getState().activePaneId;
      usePaneStore.getState().selectTab(paneId, 0);
      const layout = usePaneStore.getState().layout;
      if (layout.type === "leaf") expect(layout.activeTab).toBe(0);
    });

    it("setActivePaneId switches active pane", () => {
      usePaneStore.getState().splitHorizontal();
      const ids = allLeafIds(usePaneStore.getState().layout);
      usePaneStore.getState().setActivePaneId(ids[0]);
      expect(usePaneStore.getState().activePaneId).toBe(ids[0]);
    });

    it("resizeSplit walks a nested path", () => {
      usePaneStore.getState().splitVertical();
      usePaneStore.getState().splitHorizontal();
      // Resize root split (path [])
      usePaneStore.getState().resizeSplit([], 0.3);
      const layout = usePaneStore.getState().layout as SplitPane;
      expect(layout.ratio).toBe(0.3);
    });

    it("nextTab is no-op with 0 or 1 tabs", () => {
      usePaneStore.getState().nextTab();
      let layout = usePaneStore.getState().layout;
      if (layout.type === "leaf") expect(layout.activeTab).toBe(0);

      usePaneStore.getState().openFile("a.md", "x", VAULT);
      usePaneStore.getState().nextTab();
      layout = usePaneStore.getState().layout;
      if (layout.type === "leaf") expect(layout.activeTab).toBe(0);
    });
  });

  // ──────────────────────────────────────────────
  // Scroll positions
  // ──────────────────────────────────────────────
  describe("scroll positions", () => {
    it("setScrollPosition / getScrollPosition roundtrip", () => {
      usePaneStore.getState().setScrollPosition("a.md", 250);
      expect(usePaneStore.getState().getScrollPosition("a.md")).toBe(250);
    });

    it("getScrollPosition returns undefined for unknown file", () => {
      expect(
        usePaneStore.getState().getScrollPosition("nope.md"),
      ).toBeUndefined();
    });
  });

  // ──────────────────────────────────────────────
  // restoreLayout
  // ──────────────────────────────────────────────
  describe("restoreLayout", () => {
    it("replaces layout and activePaneId", () => {
      const newLayout = mkLeaf("restored", [
        { filePath: "x.md", vaultPath: VAULT, unsaved: false, kind: "file" },
      ]);
      usePaneStore.getState().restoreLayout(newLayout, "restored");
      expect(usePaneStore.getState().activePaneId).toBe("restored");
      expect(usePaneStore.getState().layout.type).toBe("leaf");
    });

    it("merges editor contents when provided", () => {
      const newLayout = mkLeaf("p1");
      const contents = new Map([["x.md", "# from session"]]);
      usePaneStore.getState().restoreLayout(newLayout, "p1", contents);
      expect(usePaneStore.getState().editorContents.get("x.md")).toBe(
        "# from session",
      );
    });
  });

  // ──────────────────────────────────────────────
  // Conflicts
  // ──────────────────────────────────────────────
  describe("conflicts", () => {
    it("hasConflict reflects conflictFiles set", () => {
      expect(usePaneStore.getState().hasConflict("a.md")).toBe(false);
      usePaneStore.setState({ conflictFiles: new Set(["a.md"]) });
      expect(usePaneStore.getState().hasConflict("a.md")).toBe(true);
    });

    it("resolveConflict 'reload' calls forceReloadFile and clears conflict", async () => {
      let reloadArgs: unknown = null;
      mockTauriCommand("force_reload_file", (args) => {
        reloadArgs = args;
      });
      usePaneStore.setState({ conflictFiles: new Set(["a.md"]) });

      await usePaneStore.getState().resolveConflict("a.md", "reload", VAULT);

      expect(reloadArgs).toEqual({ vaultPath: VAULT, filePath: "a.md" });
      expect(usePaneStore.getState().hasConflict("a.md")).toBe(false);
    });

    it("resolveConflict 'reload' logs error but still clears conflict", async () => {
      mockTauriCommand("force_reload_file", () => {
        throw new Error("io");
      });
      const errSpy = vi.spyOn(console, "error").mockImplementation(() => {});
      usePaneStore.setState({ conflictFiles: new Set(["a.md"]) });

      await usePaneStore.getState().resolveConflict("a.md", "reload", VAULT);

      expect(errSpy).toHaveBeenCalled();
      expect(usePaneStore.getState().hasConflict("a.md")).toBe(false);
      errSpy.mockRestore();
    });

    it("resolveConflict 'keep' just clears conflict without IO", async () => {
      let reloadCalls = 0;
      mockTauriCommand("force_reload_file", () => {
        reloadCalls++;
      });
      usePaneStore.setState({ conflictFiles: new Set(["a.md"]) });

      await usePaneStore.getState().resolveConflict("a.md", "keep", VAULT);

      expect(reloadCalls).toBe(0);
      expect(usePaneStore.getState().hasConflict("a.md")).toBe(false);
    });

    it("resolveConflict 'reload' without vaultPath skips IO", async () => {
      let reloadCalls = 0;
      mockTauriCommand("force_reload_file", () => {
        reloadCalls++;
      });
      usePaneStore.setState({ conflictFiles: new Set(["a.md"]) });

      await usePaneStore.getState().resolveConflict("a.md", "reload", null);

      expect(reloadCalls).toBe(0);
      expect(usePaneStore.getState().hasConflict("a.md")).toBe(false);
    });
  });

  // ──────────────────────────────────────────────
  // file-reloaded listener
  // ──────────────────────────────────────────────
  describe("setupPaneListeners", () => {
    it("registers listener for file-reloaded", () => {
      setupPaneListeners();
      const channels = listen.mock.calls.map((c) => c[0]);
      expect(channels).toContain("file-reloaded");
    });

    it("file-reloaded sets conflict when file is open AND unsaved", () => {
      // Open file and mark unsaved
      usePaneStore.getState().openFile("a.md", "x", VAULT);
      usePaneStore.getState().markUnsaved("p1", "a.md", true);

      setupPaneListeners();
      emitTauriEvent("file-reloaded", { path: "a.md", markdown: "external" });

      expect(usePaneStore.getState().hasConflict("a.md")).toBe(true);
    });

    it("file-reloaded ignores files not currently open", () => {
      setupPaneListeners();
      emitTauriEvent("file-reloaded", {
        path: "not-open.md",
        markdown: "x",
      });

      expect(usePaneStore.getState().hasConflict("not-open.md")).toBe(false);
    });

    it("file-reloaded does not flag conflict when file has no unsaved edits", () => {
      usePaneStore.getState().openFile("a.md", "x", VAULT);
      // No markUnsaved

      setupPaneListeners();
      emitTauriEvent("file-reloaded", { path: "a.md", markdown: "y" });

      expect(usePaneStore.getState().hasConflict("a.md")).toBe(false);
    });
  });

  describe("active tab selectors", () => {
    it("selectActiveTabPath returns null on cold start (no tabs)", () => {
      resetStore();
      const path = selectActiveTabPath(usePaneStore.getState());
      expect(path).toBeNull();
    });

    it("selectActiveTabPath returns the active leaf's active tab path", () => {
      resetStore();
      usePaneStore.getState().openFile("notes.md", "v=", VAULT);
      const path = selectActiveTabPath(usePaneStore.getState());
      expect(path).toBe("notes.md");
    });

    it("selectActiveTabPath falls back to first leaf with a tab if activePaneId is unset", () => {
      resetStore();
      usePaneStore.getState().openFile("a.md", "v=", VAULT);
      usePaneStore.setState({ activePaneId: null } as never);
      const path = selectActiveTabPath(usePaneStore.getState());
      expect(path).toBe("a.md");
    });

    it("selectActiveTabUnsaved is false when the active path is clean", () => {
      resetStore();
      usePaneStore.getState().openFile("notes.md", "v=", VAULT);
      expect(selectActiveTabUnsaved(usePaneStore.getState())).toBe(false);
    });

    it("selectActiveTabUnsaved is true when the active path is unsaved", () => {
      resetStore();
      usePaneStore.getState().openFile("notes.md", "v=", VAULT);
      usePaneStore.getState().markUnsaved("p1", "notes.md", true);
      expect(selectActiveTabUnsaved(usePaneStore.getState())).toBe(true);
    });

    it("selectActiveTabUnsaved is false when no tab is open even if some path was unsaved", () => {
      resetStore();
      usePaneStore.getState().markUnsaved("p1", "orphan.md", true);
      expect(selectActiveTabUnsaved(usePaneStore.getState())).toBe(false);
    });
  });
});
