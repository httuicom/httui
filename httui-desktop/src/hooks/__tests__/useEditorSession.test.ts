import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useEditorSession } from "@/hooks/useEditorSession";
import { usePaneStore } from "@/stores/pane";
import { useWorkspaceStore } from "@/stores/workspace";
import { useSettingsStore } from "@/stores/settings";
import { useTagIndexStore } from "@/stores/tagIndex";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";

vi.mock("@/lib/theme/apply", () => ({ applyTheme: vi.fn() }));

const VAULT = "/v";

function resetStores() {
  usePaneStore.setState({
    layout: { type: "leaf", id: "p1", tabs: [], activeTab: 0 },
    activePaneId: "p1",
    editorContents: new Map(),
    unsavedFiles: new Set(),
    scrollPositions: new Map(),
    conflictFiles: new Set(),
  });
  useWorkspaceStore.setState({
    vaultPath: VAULT,
    vaults: [],
    entries: [],
    connections: new Map(),
    activeConnection: null,
  });
  useSettingsStore.setState({
    settings: {
      autoSaveMs: 1000,
      editorFontSize: 12,
      defaultFetchSize: 80,
      historyRetention: 10,
    },
  });
}

describe("useEditorSession", () => {
  beforeEach(() => {
    resetStores();
    clearTauriMocks();
    vi.useFakeTimers();
  });

  afterEach(() => {
    clearTauriMocks();
    vi.useRealTimers();
  });

  describe("handleFileSelect", () => {
    it("reads from disk and opens when not cached", async () => {
      mockTauriCommand("read_note", () => "# fresh content");
      mockTauriCommand("set_config", () => {});

      const { result } = renderHook(() => useEditorSession());

      await act(async () => {
        await result.current.handleFileSelect("note.md");
      });

      const state = usePaneStore.getState();
      expect(state.editorContents.get("note.md")).toBe("# fresh content");
      if (state.layout.type === "leaf") {
        expect(state.layout.tabs[0].filePath).toBe("note.md");
      }
    });

    it("noop when no vault path", async () => {
      useWorkspaceStore.setState({ vaultPath: null });
      let called = false;
      mockTauriCommand("read_note", () => {
        called = true;
        return "";
      });

      const { result } = renderHook(() => useEditorSession());
      await act(async () => {
        await result.current.handleFileSelect("note.md");
      });

      expect(called).toBe(false);
    });

    it("uses cached content when available and not legacy HTML", async () => {
      usePaneStore.getState().updateContent("cached.md", "# already in memory");
      let readCalls = 0;
      mockTauriCommand("read_note", () => {
        readCalls++;
        return "should not be used";
      });
      mockTauriCommand("set_config", () => {});

      const { result } = renderHook(() => useEditorSession());

      await act(async () => {
        await result.current.handleFileSelect("cached.md");
      });

      expect(readCalls).toBe(0);
      expect(usePaneStore.getState().editorContents.get("cached.md")).toBe(
        "# already in memory",
      );
    });

    it("re-reads when cached content looks like legacy TipTap HTML", async () => {
      usePaneStore.getState().updateContent("legacy.md", "<p>old html</p>");
      let readCalls = 0;
      mockTauriCommand("read_note", () => {
        readCalls++;
        return "# markdown now";
      });
      mockTauriCommand("set_config", () => {});

      const { result } = renderHook(() => useEditorSession());

      await act(async () => {
        await result.current.handleFileSelect("legacy.md");
      });

      expect(readCalls).toBe(1);
      expect(usePaneStore.getState().editorContents.get("legacy.md")).toBe(
        "# markdown now",
      );
    });

    it("logs but does not throw on read_note error", async () => {
      mockTauriCommand("read_note", () => {
        throw new Error("ENOENT");
      });
      const errSpy = vi.spyOn(console, "error").mockImplementation(() => {});

      const { result } = renderHook(() => useEditorSession());
      await act(async () => {
        await result.current.handleFileSelect("missing.md");
      });

      expect(errSpy).toHaveBeenCalled();
      errSpy.mockRestore();
    });
  });

  describe("handleEditorChange (auto-save)", () => {
    it("updates content + marks unsaved synchronously", () => {
      const { result } = renderHook(() => useEditorSession());

      act(() => {
        result.current.handleEditorChange("p1", "a.md", "draft", VAULT);
      });

      expect(usePaneStore.getState().editorContents.get("a.md")).toBe("draft");
      expect(usePaneStore.getState().unsavedFiles.has("a.md")).toBe(true);
    });

    it("debounces write_note via autoSaveMs timer", async () => {
      let writeCalls = 0;
      let lastWrite: unknown = null;
      mockTauriCommand("write_note", (args) => {
        writeCalls++;
        lastWrite = args;
      });

      const { result } = renderHook(() => useEditorSession());

      act(() => {
        result.current.handleEditorChange("p1", "a.md", "v1", VAULT);
        result.current.handleEditorChange("p1", "a.md", "v2", VAULT);
        result.current.handleEditorChange("p1", "a.md", "v3", VAULT);
      });

      expect(writeCalls).toBe(0);

      await act(async () => {
        vi.advanceTimersByTime(1000);
        await Promise.resolve();
        await Promise.resolve();
      });

      expect(writeCalls).toBe(1);
      expect((lastWrite as { content: string }).content).toBe("v3");
      expect(usePaneStore.getState().unsavedFiles.has("a.md")).toBe(false);
    });

    it("does not save when autoSaveMs is 0", async () => {
      useSettingsStore.setState({
        settings: {
          autoSaveMs: 0,
          editorFontSize: 12,
          defaultFetchSize: 80,
          historyRetention: 10,
        },
      });
      let writeCalls = 0;
      mockTauriCommand("write_note", () => {
        writeCalls++;
      });

      const { result } = renderHook(() => useEditorSession());

      act(() => {
        result.current.handleEditorChange("p1", "a.md", "x", VAULT);
      });
      await act(async () => {
        vi.advanceTimersByTime(5000);
        await Promise.resolve();
      });

      expect(writeCalls).toBe(0);
    });

    it("skips write when file has conflict", async () => {
      let writeCalls = 0;
      mockTauriCommand("write_note", () => {
        writeCalls++;
      });

      usePaneStore.setState({ conflictFiles: new Set(["a.md"]) });

      const { result } = renderHook(() => useEditorSession());
      act(() => {
        result.current.handleEditorChange("p1", "a.md", "x", VAULT);
      });

      await act(async () => {
        vi.advanceTimersByTime(1000);
        await Promise.resolve();
        await Promise.resolve();
      });

      expect(writeCalls).toBe(0);
      expect(usePaneStore.getState().unsavedFiles.has("a.md")).toBe(true);
    });

    it("skips write when file is suppressed", async () => {
      let writeCalls = 0;
      mockTauriCommand("write_note", () => {
        writeCalls++;
      });

      const { result } = renderHook(() => useEditorSession());

      act(() => {
        result.current.handleEditorChange("p1", "a.md", "x", VAULT);
      });
      act(() => result.current.suppressAutoSave("a.md"));

      act(() => {
        result.current.handleEditorChange("p1", "a.md", "x2", VAULT);
      });

      await act(async () => {
        vi.advanceTimersByTime(1000);
        await Promise.resolve();
      });

      expect(writeCalls).toBe(0);

      act(() => result.current.unsuppressAutoSave("a.md"));
      act(() => {
        result.current.handleEditorChange("p1", "a.md", "x3", VAULT);
      });
      await act(async () => {
        vi.advanceTimersByTime(1000);
        await Promise.resolve();
        await Promise.resolve();
      });

      expect(writeCalls).toBe(1);
    });

    it("logs but does not throw on write_note error", async () => {
      mockTauriCommand("write_note", () => {
        throw new Error("disk full");
      });
      const errSpy = vi.spyOn(console, "error").mockImplementation(() => {});

      const { result } = renderHook(() => useEditorSession());
      act(() => {
        result.current.handleEditorChange("p1", "a.md", "x", VAULT);
      });
      await act(async () => {
        vi.advanceTimersByTime(1000);
        await Promise.resolve();
        await Promise.resolve();
      });

      expect(errSpy).toHaveBeenCalled();
      expect(usePaneStore.getState().unsavedFiles.has("a.md")).toBe(true);
      errSpy.mockRestore();
    });
  });

  describe("forceSave", () => {
    it("writes the active tab's content immediately", async () => {
      let written: unknown = null;
      mockTauriCommand("write_note", (args) => {
        written = args;
      });

      usePaneStore.getState().openFile("hello.md", "# content", VAULT);
      usePaneStore.getState().markUnsaved("p1", "hello.md", true);

      const { result } = renderHook(() => useEditorSession());
      act(() => result.current.forceSave());
      await act(async () => {
        await Promise.resolve();
        await Promise.resolve();
      });

      expect(written).toEqual({
        vaultPath: VAULT,
        filePath: "hello.md",
        content: "# content",
      });
      expect(usePaneStore.getState().unsavedFiles.has("hello.md")).toBe(false);
    });

    it("noop when no tabs are open", async () => {
      let writeCalls = 0;
      mockTauriCommand("write_note", () => {
        writeCalls++;
      });

      const { result } = renderHook(() => useEditorSession());
      act(() => result.current.forceSave());
      await act(async () => {
        await Promise.resolve();
      });

      expect(writeCalls).toBe(0);
    });
  });

  describe("tag-index refresh on save (epic 52 / story 04)", () => {
    it("auto-save updates the tag index from the persisted content", async () => {
      mockTauriCommand("write_note", () => {});
      useTagIndexStore.getState().clearAll();
      expect(useTagIndexStore.getState().getAllTags()).toEqual([]);

      const { result } = renderHook(() => useEditorSession());
      act(() => {
        result.current.handleEditorChange(
          "p1",
          "rb.md",
          "---\ntags: [payments, debug]\n---\nbody\n",
          VAULT,
        );
      });
      await act(async () => {
        vi.advanceTimersByTime(1000);
        await Promise.resolve();
        await Promise.resolve();
      });

      const state = useTagIndexStore.getState();
      expect(state.getAllTags()).toEqual(["debug", "payments"]);
      expect(state.getFilesByTag("payments")).toEqual(["rb.md"]);
    });

    it("forceSave updates the tag index from the persisted content", async () => {
      mockTauriCommand("write_note", () => {});
      usePaneStore.setState({
        layout: {
          type: "leaf",
          id: "p1",
          tabs: [
            { filePath: "rb.md", vaultPath: VAULT, kind: "file" } as never,
          ],
          activeTab: 0,
        } as never,
        activePaneId: "p1",
        editorContents: new Map([["rb.md", "---\ntags: [solo]\n---\nbody\n"]]),
        unsavedFiles: new Set(["rb.md"]),
      } as never);
      useTagIndexStore.getState().clearAll();

      const { result } = renderHook(() => useEditorSession());
      act(() => result.current.forceSave());
      await act(async () => {
        await Promise.resolve();
        await Promise.resolve();
      });

      expect(useTagIndexStore.getState().getAllTags()).toEqual(["solo"]);
    });

    it("flips the file's tag set when frontmatter is removed in a save", async () => {
      mockTauriCommand("write_note", () => {});
      useTagIndexStore.getState().clearAll();
      useTagIndexStore.getState().setTagsForFile("rb.md", ["legacy"]);
      expect(useTagIndexStore.getState().getAllTags()).toEqual(["legacy"]);

      const { result } = renderHook(() => useEditorSession());
      act(() => {
        result.current.handleEditorChange(
          "p1",
          "rb.md",
          "no frontmatter, body only\n",
          VAULT,
        );
      });
      await act(async () => {
        vi.advanceTimersByTime(1000);
        await Promise.resolve();
        await Promise.resolve();
      });

      expect(useTagIndexStore.getState().getAllTags()).toEqual([]);
    });
  });
});
