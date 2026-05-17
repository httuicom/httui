import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { OutlinePanel } from "@/components/layout/outline/OutlinePanel";
import { usePaneStore } from "@/stores/pane";
import { clearTauriMocks } from "@/test/mocks/tauri";
import { renderWithProviders, screen } from "@/test/render";

// CM6 active-editor registry — replace with a controllable stub so
// click-handler dispatch is observable without spinning up a real
// EditorView.
const dispatchMock = vi.fn();
const focusMock = vi.fn();
const fakeView = {
  state: { doc: { length: 1_000 } },
  dispatch: dispatchMock,
  focus: focusMock,
};

vi.mock("@/lib/codemirror/active-editor", () => ({
  getActiveEditor: () => fakeView,
}));

beforeEach(() => {
  clearTauriMocks();
  dispatchMock.mockClear();
  focusMock.mockClear();
});

afterEach(() => {
  clearTauriMocks();
});

function setActiveFile(filePath: string, content: string) {
  // The pane store layout structure expects a leaf with tabs; the
  // selectActiveTabPath selector walks layout.tabs[activeTab]. Build
  // a minimal-but-realistic shape so the selector returns filePath.
  usePaneStore.setState({
    activePaneId: "p1",
    layout: {
      type: "leaf",
      id: "p1",
      tabs: [
        {
          kind: "file",
          filePath,
          vaultPath: "/v",
          unsaved: false,
        } as never,
      ],
      activeTab: 0,
    } as never,
    editorContents: new Map([[filePath, content]]),
    unsavedFiles: new Set<string>(),
  } as never);
}

describe("OutlinePanel", () => {
  it("renders the empty state when the active file has no headings", () => {
    setActiveFile("a.md", "no heading here\n\nbody only\n");
    renderWithProviders(<OutlinePanel width={300} onClose={() => {}} />);
    // Empty headings list → OutlineList shows the "No headings yet"
    // empty state.
    expect(screen.getByTestId("outline-empty")).toBeInTheDocument();
  });

  it("renders rows for the active file's headings", () => {
    setActiveFile("a.md", "# Top\n\nbody\n\n## Section\n\nmore\n\n### Sub\n");
    renderWithProviders(<OutlinePanel width={300} onClose={() => {}} />);
    // 3 headings → 3 rows.
    const rows = screen.getAllByTestId("outline-row");
    expect(rows).toHaveLength(3);
  });

  it("dispatches a CM6 selection on row click and focuses the view", async () => {
    setActiveFile("a.md", "# Hello\n\nbody\n");
    renderWithProviders(<OutlinePanel width={300} onClose={() => {}} />);
    const row = screen.getAllByTestId("outline-row")[0]!;
    await userEvent.click(row);
    expect(dispatchMock).toHaveBeenCalledTimes(1);
    expect(focusMock).toHaveBeenCalledTimes(1);
    const arg = dispatchMock.mock.calls[0]?.[0] as {
      selection: { anchor: number };
      scrollIntoView: boolean;
    };
    // First heading is at offset 0.
    expect(arg.selection.anchor).toBe(0);
    expect(arg.scrollIntoView).toBe(true);
  });

  it("clamps stale offsets against the current doc length", async () => {
    // Outline parsed against the long content — the click later
    // dispatches against a mock view whose `doc.length === 1000`. If
    // a heading offset somehow exceeds 1000 we'd land past EOD; the
    // panel clamps to doc.length so the dispatch stays safe.
    const longHeader = "# " + "x".repeat(2000) + "\n";
    setActiveFile("a.md", longHeader);
    renderWithProviders(<OutlinePanel width={300} onClose={() => {}} />);
    const row = screen.getAllByTestId("outline-row")[0]!;
    await userEvent.click(row);
    const arg = dispatchMock.mock.calls[0]?.[0] as {
      selection: { anchor: number };
    };
    expect(arg.selection.anchor).toBeLessThanOrEqual(1_000);
  });

  it("close button fires onClose", async () => {
    setActiveFile("a.md", "# Hi\n");
    const onClose = vi.fn();
    renderWithProviders(<OutlinePanel width={300} onClose={onClose} />);
    const closeBtn = screen.getByLabelText("Close outline panel");
    await userEvent.click(closeBtn);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("renders empty state when no file is active", () => {
    usePaneStore.setState({
      activePaneId: null,
      layout: {
        type: "leaf",
        id: "p1",
        tabs: [],
        activeTab: 0,
      } as never,
      editorContents: new Map(),
      unsavedFiles: new Set<string>(),
    } as never);
    renderWithProviders(<OutlinePanel width={300} onClose={() => {}} />);
    expect(screen.getByTestId("outline-empty")).toBeInTheDocument();
  });
});
