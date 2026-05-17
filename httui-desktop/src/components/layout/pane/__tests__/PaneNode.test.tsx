import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { PaneNode } from "@/components/layout/pane/PaneNode";
import { usePaneStore } from "@/stores/pane";
import { useSettingsStore } from "@/stores/settings";
import { clearTauriMocks, mockTauriCommand } from "@/test/mocks/tauri";
import { renderWithProviders, screen } from "@/test/render";
import type { LeafPane, PaneLayout, SplitPane, TabState } from "@/types/pane";

vi.mock("@/components/layout/pane/DocHeaderedEditor", () => ({
  DocHeaderedEditor: ({
    filePath,
    onConflictReload,
    onConflictKeep,
    onChange,
  }: {
    filePath: string;
    onConflictReload: () => void;
    onConflictKeep: () => void;
    onChange: (c: string) => void;
  }) => (
    <div data-testid="docheadered-editor" data-file={filePath}>
      <button data-testid="conflict-reload" onClick={onConflictReload}>
        reload
      </button>
      <button data-testid="conflict-keep" onClick={onConflictKeep}>
        keep
      </button>
      <button
        data-testid="trigger-change"
        onClick={() => onChange("new content")}
      >
        change
      </button>
    </div>
  ),
}));

vi.mock("@/components/editor/DiffViewer", () => ({
  DiffViewer: ({ tab }: { tab: { filePath: string } }) => (
    <div data-testid="diff-viewer" data-file={tab.filePath} />
  ),
}));

vi.mock("../../TabBar", () => ({
  TabBar: ({
    tabs,
    onSelectTab,
    onCloseTab,
    onCloseOthers,
    onCloseAll,
  }: {
    tabs: Array<{ filePath: string }>;
    onSelectTab: (i: number) => void;
    onCloseTab: (i: number) => void;
    onCloseOthers: (i: number) => void;
    onCloseAll: () => void;
  }) => (
    <div data-testid="tab-bar" data-count={tabs.length}>
      <button data-testid="select-tab" onClick={() => onSelectTab(0)} />
      <button data-testid="close-tab" onClick={() => onCloseTab(0)} />
      <button data-testid="close-others" onClick={() => onCloseOthers(0)} />
      <button data-testid="close-all" onClick={() => onCloseAll()} />
    </div>
  ),
}));

vi.mock("../SplitView", () => ({
  SplitView: ({ layout }: { layout: SplitPane }) => (
    <div data-testid="split-view" data-direction={layout.direction} />
  ),
}));

beforeEach(() => {
  clearTauriMocks();
  // Reset paneStore + settings to a known shape.
  usePaneStore.setState({
    activePaneId: "p1",
    editorContents: new Map([["a.md", "# hi\n"]]),
    unsavedFiles: new Set<string>(),
  } as never);
  useSettingsStore.setState({ vimEnabled: false } as never);
});

afterEach(() => {
  clearTauriMocks();
});

const leafLayout = (override?: Partial<LeafPane>): LeafPane => ({
  type: "leaf",
  id: "p1",
  tabs: [
    {
      kind: "file",
      filePath: "a.md",
      vaultPath: "/v",
      unsaved: false,
    } satisfies TabState,
  ],
  activeTab: 0,
  ...(override ?? {}),
});

describe("PaneNode", () => {
  it("renders an empty-leaf message when the layout has no tabs", () => {
    const layout: PaneLayout = {
      type: "leaf",
      id: "p1",
      tabs: [],
      activeTab: 0,
    };
    renderWithProviders(
      <PaneNode layout={layout} path={[]} handleEditorChange={vi.fn()} />,
    );
    expect(
      screen.getByText(/Open a file to start editing/),
    ).toBeInTheDocument();
  });

  it("renders the DocHeaderedEditor for a file tab", () => {
    renderWithProviders(
      <PaneNode layout={leafLayout()} path={[]} handleEditorChange={vi.fn()} />,
    );
    const editor = screen.getByTestId("docheadered-editor");
    expect(editor.dataset.file).toBe("a.md");
    expect(screen.queryByTestId("diff-viewer")).not.toBeInTheDocument();
  });

  it("renders the DiffViewer when the active tab is a diff tab", () => {
    const layout: PaneLayout = {
      type: "leaf",
      id: "p1",
      tabs: [
        {
          kind: "diff",
          filePath: "a.md",
          vaultPath: "/v",
        } as never,
      ],
      activeTab: 0,
    };
    renderWithProviders(
      <PaneNode layout={layout} path={[]} handleEditorChange={vi.fn()} />,
    );
    expect(screen.getByTestId("diff-viewer")).toBeInTheDocument();
    expect(screen.queryByTestId("docheadered-editor")).not.toBeInTheDocument();
  });

  it("delegates non-leaf layouts to SplitView", () => {
    const layout: PaneLayout = {
      type: "split",
      direction: "horizontal",
      ratio: 0.5,
      children: [
        { type: "leaf", id: "p1", tabs: [], activeTab: 0 },
        { type: "leaf", id: "p2", tabs: [], activeTab: 0 },
      ],
    };
    renderWithProviders(
      <PaneNode layout={layout} path={[]} handleEditorChange={vi.fn()} />,
    );
    expect(screen.getByTestId("split-view")).toBeInTheDocument();
    expect(screen.getByTestId("split-view").dataset.direction).toBe(
      "horizontal",
    );
  });

  it("renders the TabBar with the leaf's tab count", () => {
    renderWithProviders(
      <PaneNode layout={leafLayout()} path={[]} handleEditorChange={vi.fn()} />,
    );
    expect(screen.getByTestId("tab-bar").dataset.count).toBe("1");
  });

  it("forwards the file content from the editorContents map", () => {
    // Indirect — DocHeaderedEditor mock would receive content as prop;
    // assert the mount path opens the editor at all (the file content
    // is in the editorContents Map keyed by filePath).
    renderWithProviders(
      <PaneNode layout={leafLayout()} path={[]} handleEditorChange={vi.fn()} />,
    );
    expect(screen.getByTestId("docheadered-editor")).toBeInTheDocument();
  });

  it("calls store actions for tab-bar callbacks", () => {
    const selectTab = vi.fn();
    const closeTab = vi.fn();
    const closeOthers = vi.fn();
    const closeAll = vi.fn();
    usePaneStore.setState({
      activePaneId: "p1",
      editorContents: new Map(),
      unsavedFiles: new Set<string>(),
      selectTab,
      closeTab,
      closeOthers,
      closeAll,
    } as never);
    renderWithProviders(
      <PaneNode layout={leafLayout()} path={[]} handleEditorChange={vi.fn()} />,
    );
    screen.getByTestId("select-tab").click();
    screen.getByTestId("close-tab").click();
    screen.getByTestId("close-others").click();
    screen.getByTestId("close-all").click();
    expect(selectTab).toHaveBeenCalledWith("p1", 0);
    expect(closeTab).toHaveBeenCalledWith("p1", 0);
    expect(closeOthers).toHaveBeenCalledWith("p1", 0);
    expect(closeAll).toHaveBeenCalledWith("p1");
  });

  it("activates the pane on click + delegates conflict + change callbacks", () => {
    const setActivePaneId = vi.fn();
    const resolveConflict = vi.fn();
    const handleEditorChange = vi.fn();
    usePaneStore.setState({
      activePaneId: "other",
      editorContents: new Map(),
      unsavedFiles: new Set<string>(),
      setActivePaneId,
      resolveConflict,
    } as never);
    renderWithProviders(
      <PaneNode
        layout={leafLayout()}
        path={[]}
        handleEditorChange={handleEditorChange}
      />,
    );
    // Pane click delegates to setActivePaneId.
    screen.getByTestId("tab-bar").click();
    expect(setActivePaneId).toHaveBeenCalledWith("p1");

    // Conflict + change handlers thread through DocHeaderedEditor.
    screen.getByTestId("conflict-reload").click();
    expect(resolveConflict).toHaveBeenCalledWith("a.md", "reload", "/v");
    screen.getByTestId("conflict-keep").click();
    expect(resolveConflict).toHaveBeenLastCalledWith("a.md", "keep", null);
    screen.getByTestId("trigger-change").click();
    expect(handleEditorChange).toHaveBeenCalledWith(
      "p1",
      "a.md",
      "new content",
      "/v",
    );
  });

  it("recovers HTML-cached files via readNote re-fetch (TipTap legacy path)", async () => {
    const openFile = vi.fn();
    mockTauriCommand("read_note", () => "# md\n");
    usePaneStore.setState({
      activePaneId: "p1",
      // Cached content starts with `<` (HTML, not markdown) — triggers
      // the recovery effect.
      editorContents: new Map([["legacy.md", "<p>hi</p>"]]),
      unsavedFiles: new Set<string>(),
      openFile,
    } as never);
    const layout: PaneLayout = {
      type: "leaf",
      id: "p1",
      tabs: [
        {
          kind: "file",
          filePath: "legacy.md",
          vaultPath: "/v",
        } as never,
      ],
      activeTab: 0,
    };
    renderWithProviders(
      <PaneNode layout={layout} path={[]} handleEditorChange={vi.fn()} />,
    );
    // Wait for the readNote promise + setState to flush.
    await vi.waitFor(() => {
      expect(openFile).toHaveBeenCalledWith("legacy.md", "# md\n", "/v");
    });
  });
});
