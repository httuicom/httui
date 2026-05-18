import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ChakraProvider, defaultSystem } from "@chakra-ui/react";
import { render, screen, fireEvent } from "@testing-library/react";
import type { ReactNode } from "react";

import {
  WorkspaceContext,
  type WorkspaceContextValue,
} from "@/contexts/WorkspaceContext";
import { FileTree } from "@/components/layout/file-tree/FileTree";
import { useTagIndexStore } from "@/stores/tagIndex";
import { useArchiveFilterStore } from "@/stores/archiveFilter";
import { usePaneStore } from "@/stores/pane";
import type { FileEntry } from "@/lib/tauri/commands";

vi.mock("@/lib/theme/apply", () => ({ applyTheme: vi.fn() }));

function makeWorkspaceStub(
  over: Partial<WorkspaceContextValue> = {},
): WorkspaceContextValue {
  return {
    vaultPath: "/v",
    vaults: [],
    entries: [],
    switchVault: vi.fn(async () => {}),
    openVault: vi.fn(async () => {}),
    inlineCreate: null,
    handleStartCreate: vi.fn(),
    handleCreateNote: vi.fn(async () => {}),
    handleCreateFolder: vi.fn(async () => {}),
    handleRename: vi.fn(async () => {}),
    handleDelete: vi.fn(async () => {}),
    handleMoveFile: vi.fn(async () => {}),
    cancelInlineCreate: vi.fn(),
    handleFileSelect: vi.fn(async () => {}),
    ...over,
  };
}

function renderTree(workspace: Partial<WorkspaceContextValue> = {}) {
  const value = makeWorkspaceStub(workspace);
  function Wrap({ children }: { children: ReactNode }) {
    return (
      <ChakraProvider value={defaultSystem}>
        <WorkspaceContext.Provider value={value}>
          {children}
        </WorkspaceContext.Provider>
      </ChakraProvider>
    );
  }
  return render(<FileTree />, { wrapper: Wrap });
}

const note: FileEntry = {
  name: "note.md",
  path: "/v/note.md",
  is_dir: false,
  children: null,
};

describe("FileTree", () => {
  beforeEach(() => {
    useTagIndexStore.getState().clearAll();
    useArchiveFilterStore.setState({ showArchived: false });
    usePaneStore.setState({
      layout: { type: "leaf", id: "p1", tabs: [], activeTab: 0 },
      activePaneId: "p1",
      editorContents: new Map(),
      unsavedFiles: new Set(),
      scrollPositions: new Map(),
      conflictFiles: new Set(),
    });
  });

  afterEach(() => {
    if (typeof localStorage !== "undefined") {
      localStorage.removeItem("archive-filter");
    }
  });

  it("renders 'Empty vault' placeholder when there are no entries", () => {
    renderTree({ entries: [] });
    expect(screen.getByText("Empty vault")).toBeInTheDocument();
  });

  it("renders the entries when present", () => {
    renderTree({ entries: [note] });
    expect(screen.getByText("note")).toBeInTheDocument();
  });

  describe("ArchiveFilterToggle", () => {
    it("hides the toggle when no archived files exist", () => {
      renderTree({ entries: [note] });
      expect(
        screen.queryByTestId("file-tree-archive-toggle"),
      ).not.toBeInTheDocument();
    });

    it("shows the toggle with the archived count when there are archived files", () => {
      useTagIndexStore.getState().setArchivedForFile("/v/old.md", true);
      useTagIndexStore.getState().setArchivedForFile("/v/older.md", true);
      renderTree({ entries: [note] });
      const toggle = screen.getByTestId("file-tree-archive-toggle");
      expect(toggle).toBeInTheDocument();
      expect(toggle.textContent).toContain("Show archived (2)");
    });

    it("flips showArchived on click", () => {
      useTagIndexStore.getState().setArchivedForFile("/v/x.md", true);
      renderTree({ entries: [note] });
      const toggle = screen.getByTestId("file-tree-archive-toggle");
      expect(useArchiveFilterStore.getState().showArchived).toBe(false);
      fireEvent.click(toggle);
      expect(useArchiveFilterStore.getState().showArchived).toBe(true);
    });

    it("swaps copy + icon between hide / show states", () => {
      useTagIndexStore.getState().setArchivedForFile("/v/x.md", true);
      useArchiveFilterStore.setState({ showArchived: true });
      renderTree({ entries: [note] });
      const toggle = screen.getByTestId("file-tree-archive-toggle");
      expect(toggle.textContent).toContain("Hide archived");
      expect(toggle.getAttribute("data-show-archived")).toBe("true");
    });
  });

  describe("inline create at root", () => {
    it("renders the inline input when inlineCreate.dirPath is empty", () => {
      renderTree({
        entries: [note],
        inlineCreate: { type: "note", dirPath: "" },
      });
      expect(screen.getByRole("textbox")).toBeInTheDocument();
    });

    it("dispatches handleCreateNote on confirm", () => {
      const handleCreateNote = vi.fn(async () => {});
      renderTree({
        entries: [],
        inlineCreate: { type: "note", dirPath: "" },
        handleCreateNote,
      });
      const input = screen.getByRole("textbox") as HTMLInputElement;
      fireEvent.change(input, { target: { value: "new.md" } });
      fireEvent.keyDown(input, { key: "Enter" });
      expect(handleCreateNote).toHaveBeenCalledWith("", "new.md");
    });

    it("dispatches handleCreateFolder when type is folder", () => {
      const handleCreateFolder = vi.fn(async () => {});
      renderTree({
        entries: [],
        inlineCreate: { type: "folder", dirPath: "" },
        handleCreateFolder,
      });
      const input = screen.getByRole("textbox") as HTMLInputElement;
      fireEvent.change(input, { target: { value: "newdir" } });
      fireEvent.keyDown(input, { key: "Enter" });
      expect(handleCreateFolder).toHaveBeenCalledWith("", "newdir");
    });

    it("hides 'Empty vault' when an inline create is active even with no entries", () => {
      renderTree({
        entries: [],
        inlineCreate: { type: "note", dirPath: "" },
      });
      expect(screen.queryByText("Empty vault")).not.toBeInTheDocument();
    });
  });
});
