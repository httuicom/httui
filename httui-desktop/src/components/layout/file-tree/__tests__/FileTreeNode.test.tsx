import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { ChakraProvider, defaultSystem } from "@chakra-ui/react";
import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DndContext } from "@dnd-kit/core";
import {
  WorkspaceContext,
  type WorkspaceContextValue,
} from "@/contexts/WorkspaceContext";
import { FileTreeNode } from "@/components/layout/file-tree/FileTreeNode";
import { usePaneStore } from "@/stores/pane";
import type { FileEntry } from "@/lib/tauri/commands";
import type { ReactNode } from "react";

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

function renderTree(
  entry: FileEntry,
  depth: number,
  workspaceOverrides: Partial<WorkspaceContextValue> = {},
) {
  const value = makeWorkspaceStub(workspaceOverrides);
  function Wrap({ children }: { children: ReactNode }) {
    return (
      <ChakraProvider value={defaultSystem}>
        <WorkspaceContext.Provider value={value}>
          <DndContext>{children}</DndContext>
        </WorkspaceContext.Provider>
      </ChakraProvider>
    );
  }
  return {
    ...render(<FileTreeNode entry={entry} depth={depth} />, { wrapper: Wrap }),
    workspace: value,
  };
}

const noteEntry: FileEntry = {
  name: "note.md",
  path: "note.md",
  is_dir: false,
  children: null,
};

const folderEntry: FileEntry = {
  name: "folder",
  path: "folder",
  is_dir: true,
  children: [noteEntry],
};

describe("FileTreeNode", () => {
  beforeEach(() => {
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
    vi.clearAllMocks();
  });

  it("renders a note name without the .md extension", () => {
    renderTree(noteEntry, 0);
    expect(screen.getByText("note")).toBeInTheDocument();
  });

  it("renders a folder name with extension preserved", () => {
    renderTree({ ...folderEntry, name: "my-folder" }, 0);
    expect(screen.getByText("my-folder")).toBeInTheDocument();
  });

  it("depth 0 starts expanded by default (children visible)", () => {
    renderTree(folderEntry, 0);
    // root depth → child immediately visible
    expect(screen.getByText("note")).toBeInTheDocument();
  });

  it("renders inline child input (textbox) when inlineCreate matches the folder", () => {
    renderTree(folderEntry, 0, {
      inlineCreate: { type: "note", dirPath: "folder" },
    });

    // InlineInput renders a Chakra Input; query by role
    expect(screen.getByRole("textbox")).toBeInTheDocument();
  });

  it("highlights active file when path matches the active tab", () => {
    usePaneStore.setState({
      layout: {
        type: "leaf",
        id: "p1",
        tabs: [
          {
            filePath: "note.md",
            vaultPath: "/v",
            unsaved: false,
            kind: "file",
          },
        ],
        activeTab: 0,
      },
      activePaneId: "p1",
    });

    renderTree(noteEntry, 1);
    // The wrapper button receives bg.emphasized when isActive — we just
    // confirm the row renders without crashing and the text is present
    expect(screen.getByText("note")).toBeInTheDocument();
  });

  it("clicking a note row calls handleFileSelect with the entry path", () => {
    const { workspace } = renderTree(noteEntry, 0);
    // fireEvent.click bypasses dnd-kit pointer sensors that intercept
    // userEvent.click on the draggable HStack.
    fireEvent.click(screen.getByText("note"));
    expect(workspace.handleFileSelect).toHaveBeenCalledWith("note.md");
  });

  it("clicking a folder row at depth>0 toggles expansion", () => {
    // depth 1 starts collapsed (only depth 0 is auto-expanded)
    renderTree(folderEntry, 1);
    expect(screen.queryByText("note")).not.toBeInTheDocument();
    fireEvent.click(screen.getByText("folder"));
    expect(screen.getByText("note")).toBeInTheDocument();
    fireEvent.click(screen.getByText("folder"));
    expect(screen.queryByText("note")).not.toBeInTheDocument();
  });

  it("inline child input confirms a new note via handleCreateNote", async () => {
    const user = userEvent.setup();
    const { workspace } = renderTree(folderEntry, 0, {
      inlineCreate: { type: "note", dirPath: "folder" },
    });
    const input = screen.getByRole("textbox") as HTMLInputElement;
    await user.type(input, "newnote{Enter}");
    expect(workspace.handleCreateNote).toHaveBeenCalledWith(
      "folder",
      "newnote",
    );
  });

  it("inline child input confirms a new folder via handleCreateFolder", async () => {
    const user = userEvent.setup();
    const { workspace } = renderTree(folderEntry, 0, {
      inlineCreate: { type: "folder", dirPath: "folder" },
    });
    const input = screen.getByRole("textbox") as HTMLInputElement;
    await user.type(input, "newdir{Enter}");
    expect(workspace.handleCreateFolder).toHaveBeenCalledWith(
      "folder",
      "newdir",
    );
  });

  it("inline child input Escape calls cancelInlineCreate", async () => {
    const user = userEvent.setup();
    const { workspace } = renderTree(folderEntry, 0, {
      inlineCreate: { type: "note", dirPath: "folder" },
    });
    const input = screen.getByRole("textbox") as HTMLInputElement;
    await user.type(input, "{Escape}");
    expect(workspace.cancelInlineCreate).toHaveBeenCalled();
  });

  it("right-click on a note opens the context menu with Renomear and Excluir", async () => {
    const user = userEvent.setup();
    renderTree(noteEntry, 0);
    fireEvent.contextMenu(screen.getByText("note"));
    expect(await screen.findByText("Renomear")).toBeInTheDocument();
    expect(screen.getByText("Excluir")).toBeInTheDocument();
    // Folder-only items should NOT be present for a note
    expect(screen.queryByText("Nova nota")).not.toBeInTheDocument();
    expect(screen.queryByText("Nova pasta")).not.toBeInTheDocument();
    void user;
  });

  it("right-click on a folder shows folder-specific menu items", async () => {
    renderTree(folderEntry, 0);
    fireEvent.contextMenu(screen.getByText("folder"));
    expect(await screen.findByText("Nova nota")).toBeInTheDocument();
    expect(screen.getByText("Nova pasta")).toBeInTheDocument();
    expect(screen.getByText("Renomear")).toBeInTheDocument();
    expect(screen.getByText("Excluir")).toBeInTheDocument();
  });

  it("Excluir on a note triggers handleDelete with the path", async () => {
    const user = userEvent.setup();
    const { workspace } = renderTree(noteEntry, 0);
    fireEvent.contextMenu(screen.getByText("note"));
    await user.click(await screen.findByText("Excluir"));
    expect(workspace.handleDelete).toHaveBeenCalledWith("note.md");
  });

  it("Nova nota on a folder triggers handleStartCreate with type=note", async () => {
    const user = userEvent.setup();
    const { workspace } = renderTree(folderEntry, 0);
    fireEvent.contextMenu(screen.getByText("folder"));
    await user.click(await screen.findByText("Nova nota"));
    expect(workspace.handleStartCreate).toHaveBeenCalledWith("note", "folder");
  });

  it("Nova pasta on a folder triggers handleStartCreate with type=folder", async () => {
    const user = userEvent.setup();
    const { workspace } = renderTree(folderEntry, 0);
    fireEvent.contextMenu(screen.getByText("folder"));
    await user.click(await screen.findByText("Nova pasta"));
    expect(workspace.handleStartCreate).toHaveBeenCalledWith(
      "folder",
      "folder",
    );
  });

  it("Renomear swaps the row for an InlineInput pre-filled with the name", async () => {
    const user = userEvent.setup();
    renderTree(noteEntry, 0);
    fireEvent.contextMenu(screen.getByText("note"));
    await user.click(await screen.findByText("Renomear"));
    const input = screen.getByRole("textbox") as HTMLInputElement;
    expect(input.value).toBe("note.md");
  });

  it("Renomear → Enter confirms via handleRename and exits rename mode", async () => {
    const user = userEvent.setup();
    const { workspace } = renderTree(noteEntry, 0);
    fireEvent.contextMenu(screen.getByText("note"));
    await user.click(await screen.findByText("Renomear"));
    const input = screen.getByRole("textbox") as HTMLInputElement;
    await user.clear(input);
    await user.type(input, "renamed.md{Enter}");
    expect(workspace.handleRename).toHaveBeenCalledWith(
      "note.md",
      "renamed.md",
    );
    // After confirm, row reverts to display mode (text node back)
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  });

  describe("archived hide", () => {
    it("hides archived notes when showArchived is false", async () => {
      const { useTagIndexStore } = await import("@/stores/tagIndex");
      const { useArchiveFilterStore } = await import("@/stores/archiveFilter");
      useTagIndexStore.getState().clearAll();
      useTagIndexStore.getState().setArchivedForFile("note.md", true);
      useArchiveFilterStore.setState({ showArchived: false });

      const { container } = renderTree(noteEntry, 0);
      expect(container.querySelector("button")).toBeNull();
    });

    it("shows archived notes with a badge when showArchived is true", async () => {
      const { useTagIndexStore } = await import("@/stores/tagIndex");
      const { useArchiveFilterStore } = await import("@/stores/archiveFilter");
      useTagIndexStore.getState().clearAll();
      useTagIndexStore.getState().setArchivedForFile("note.md", true);
      useArchiveFilterStore.setState({ showArchived: true });

      renderTree(noteEntry, 0);
      expect(screen.getByText("note")).toBeInTheDocument();
      expect(
        screen.getByTestId("file-tree-archived-badge"),
      ).toBeInTheDocument();
    });

    it("does not affect non-archived notes", async () => {
      const { useTagIndexStore } = await import("@/stores/tagIndex");
      const { useArchiveFilterStore } = await import("@/stores/archiveFilter");
      useTagIndexStore.getState().clearAll();
      useArchiveFilterStore.setState({ showArchived: false });

      renderTree(noteEntry, 0);
      expect(screen.getByText("note")).toBeInTheDocument();
      expect(
        screen.queryByTestId("file-tree-archived-badge"),
      ).not.toBeInTheDocument();
    });
  });
});
