import { describe, it, expect, beforeEach } from "vitest";
import { usePaneStore } from "@/stores/pane";

const V = "/test-vault";

describe("paneStore", () => {
  beforeEach(() => {
    // Reset store to initial state before each test
    usePaneStore.setState({
      layout: { type: "leaf", id: "test-pane-1", tabs: [], activeTab: 0 },
      activePaneId: "test-pane-1",
      editorContents: new Map(),
      unsavedFiles: new Set(),
      scrollPositions: new Map(),
      conflictFiles: new Set(),
    });
  });

  it("starts with a single empty leaf pane", () => {
    const { layout } = usePaneStore.getState();
    expect(layout.type).toBe("leaf");
    if (layout.type === "leaf") {
      expect(layout.tabs).toHaveLength(0);
    }
  });

  it("openFile adds a tab to the active pane", () => {
    usePaneStore.getState().openFile("test.md", "<p>hello</p>", V);
    const { layout } = usePaneStore.getState();
    if (layout.type === "leaf") {
      expect(layout.tabs).toHaveLength(1);
      expect(layout.tabs[0].filePath).toBe("test.md");
      expect(layout.tabs[0].vaultPath).toBe(V);
      expect(layout.activeTab).toBe(0);
    }
  });

  it("openFile switches to existing tab instead of duplicating", () => {
    const store = usePaneStore.getState();
    store.openFile("a.md", "a", V);
    usePaneStore.getState().openFile("b.md", "b", V);
    usePaneStore.getState().openFile("a.md", "a", V);
    const { layout } = usePaneStore.getState();
    if (layout.type === "leaf") {
      expect(layout.tabs).toHaveLength(2);
      expect(layout.activeTab).toBe(0);
    }
  });

  it("closeTab removes a tab", () => {
    usePaneStore.getState().openFile("a.md", "a", V);
    usePaneStore.getState().openFile("b.md", "b", V);
    const { activePaneId } = usePaneStore.getState();
    usePaneStore.getState().closeTab(activePaneId, 0);
    const { layout } = usePaneStore.getState();
    if (layout.type === "leaf") {
      expect(layout.tabs).toHaveLength(1);
      expect(layout.tabs[0].filePath).toBe("b.md");
    }
  });

  it("splitVertical creates a split layout", () => {
    usePaneStore.getState().splitVertical();
    const { layout } = usePaneStore.getState();
    expect(layout.type).toBe("split");
    if (layout.type === "split") {
      expect(layout.direction).toBe("vertical");
      expect(layout.ratio).toBe(0.5);
    }
  });

  it("splitHorizontal creates a horizontal split", () => {
    usePaneStore.getState().splitHorizontal();
    const { layout } = usePaneStore.getState();
    expect(layout.type).toBe("split");
    if (layout.type === "split") {
      expect(layout.direction).toBe("horizontal");
    }
  });

  it("markUnsaved toggles unsavedFiles set", () => {
    usePaneStore.getState().openFile("a.md", "a", V);
    const { activePaneId } = usePaneStore.getState();
    usePaneStore.getState().markUnsaved(activePaneId, "a.md", true);
    expect(usePaneStore.getState().unsavedFiles.has("a.md")).toBe(true);
    usePaneStore.getState().markUnsaved(activePaneId, "a.md", false);
    expect(usePaneStore.getState().unsavedFiles.has("a.md")).toBe(false);
  });

  it("nextTab cycles to next tab", () => {
    usePaneStore.getState().openFile("a.md", "a", V);
    usePaneStore.getState().openFile("b.md", "b", V);
    usePaneStore.getState().openFile("c.md", "c", V);
    usePaneStore.getState().nextTab();
    const { layout } = usePaneStore.getState();
    if (layout.type === "leaf") {
      expect(layout.activeTab).toBe(0);
    }
  });

  it("closeOthers keeps only the specified tab", () => {
    usePaneStore.getState().openFile("a.md", "a", V);
    usePaneStore.getState().openFile("b.md", "b", V);
    usePaneStore.getState().openFile("c.md", "c", V);
    const { activePaneId } = usePaneStore.getState();
    usePaneStore.getState().closeOthers(activePaneId, 1);
    const { layout } = usePaneStore.getState();
    if (layout.type === "leaf") {
      expect(layout.tabs).toHaveLength(1);
      expect(layout.tabs[0].filePath).toBe("b.md");
    }
  });

  it("closeAll removes all tabs", () => {
    usePaneStore.getState().openFile("a.md", "a", V);
    usePaneStore.getState().openFile("b.md", "b", V);
    const { activePaneId } = usePaneStore.getState();
    usePaneStore.getState().closeAll(activePaneId);
    const { layout } = usePaneStore.getState();
    if (layout.type === "leaf") {
      expect(layout.tabs).toHaveLength(0);
    }
  });

  it("resizeSplit changes split ratio", () => {
    usePaneStore.getState().splitVertical();
    usePaneStore.getState().resizeSplit([], 0.7);
    const { layout } = usePaneStore.getState();
    if (layout.type === "split") {
      expect(layout.ratio).toBe(0.7);
    }
  });

  it("editorContents stores content in Zustand state", () => {
    usePaneStore.getState().openFile("test.md", "<p>content</p>", V);
    expect(usePaneStore.getState().editorContents.get("test.md")).toBe(
      "<p>content</p>",
    );
    usePaneStore.getState().updateContent("test.md", "<p>updated</p>");
    expect(usePaneStore.getState().editorContents.get("test.md")).toBe(
      "<p>updated</p>",
    );
  });
});
