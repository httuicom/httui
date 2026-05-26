import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";
import { useSessionPersistence } from "@/hooks/useSessionPersistence";
import { usePaneStore } from "@/stores/pane";
import { useWorkspaceStore } from "@/stores/workspace";
import { useSettingsStore } from "@/stores/settings";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";
import type { SessionState, FileEntry } from "@/lib/tauri/commands";

vi.mock("@/lib/theme/apply", () => ({ applyTheme: vi.fn() }));

const VAULT = "/v";

const mkSession = (over?: Partial<SessionState>): SessionState => ({
  vaults: ["/v"],
  active_vault: VAULT,
  vim_enabled: false,
  sidebar_open: true,
  pane_layout: null,
  active_pane_id: null,
  active_file: null,
  scroll_positions: null,
  file_tree: [] as FileEntry[],
  tab_contents: [],
  ...over,
});

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
    vaultPath: null,
    vaults: [],
    entries: [],
    connections: new Map(),
    activeConnection: null,
  });
  useSettingsStore.setState({
    sidebarOpen: true,
    vimEnabled: false,
  });
}

describe("useSessionPersistence", () => {
  beforeEach(() => {
    resetStores();
    clearTauriMocks();
    // Default no-op mocks for fire-and-forget IPCs
    mockTauriCommand("start_watching", () => {});
    mockTauriCommand("rebuild_search_index", () => {});
    mockTauriCommand("set_config", () => {});
  });

  afterEach(() => {
    clearTauriMocks();
  });

  describe("restoreSession on mount", () => {
    it("populates vaults, active vault and file tree", async () => {
      const tree: FileEntry[] = [
        { name: "hi.md", path: "hi.md", is_dir: false, children: null },
      ];
      mockTauriCommand("restore_session", () =>
        mkSession({ vaults: ["/v", "/w"], file_tree: tree }),
      );

      renderHook(() => useSessionPersistence());

      await waitFor(() => {
        expect(useWorkspaceStore.getState().vaultPath).toBe(VAULT);
      });
      expect(useWorkspaceStore.getState().vaults).toEqual(["/v", "/w"]);
      expect(useWorkspaceStore.getState().entries).toEqual(tree);
    });

    it("sets vim_enabled when session says so", async () => {
      mockTauriCommand("restore_session", () =>
        mkSession({ vim_enabled: true }),
      );

      renderHook(() => useSessionPersistence());

      await waitFor(() => {
        expect(useSettingsStore.getState().vimEnabled).toBe(true);
      });
    });

    it("sets sidebarOpen from session", async () => {
      mockTauriCommand("restore_session", () =>
        mkSession({ sidebar_open: false }),
      );

      renderHook(() => useSessionPersistence());

      await waitFor(() => {
        expect(useSettingsStore.getState().sidebarOpen).toBe(false);
      });
    });

    it("opens single active_file when no pane_layout", async () => {
      mockTauriCommand("restore_session", () =>
        mkSession({
          active_file: "single.md",
          tab_contents: [
            {
              file_path: "single.md",
              vault_path: VAULT,
              content: "# hello",
            },
          ],
        }),
      );

      renderHook(() => useSessionPersistence());

      await waitFor(() => {
        const layout = usePaneStore.getState().layout;
        expect(layout.type === "leaf" && layout.tabs.length).toBe(1);
      });
    });

    it("restores pane_layout JSON when present", async () => {
      const restoredLayout = {
        type: "leaf",
        id: "restored",
        tabs: [{ filePath: "a.md", vaultPath: VAULT, kind: "file" as const }],
        activeTab: 0,
      };
      mockTauriCommand("restore_session", () =>
        mkSession({
          pane_layout: JSON.stringify(restoredLayout),
          active_pane_id: "restored",
          tab_contents: [
            { file_path: "a.md", vault_path: VAULT, content: "# A" },
          ],
        }),
      );

      renderHook(() => useSessionPersistence());

      await waitFor(() => {
        expect(usePaneStore.getState().activePaneId).toBe("restored");
      });
      const layout = usePaneStore.getState().layout;
      if (layout.type === "leaf") {
        expect(layout.tabs[0].filePath).toBe("a.md");
      }
    });

    it("filters tabs whose files are missing from tab_contents", async () => {
      const restoredLayout = {
        type: "leaf",
        id: "p1",
        tabs: [
          { filePath: "exists.md", vaultPath: VAULT, kind: "file" as const },
          { filePath: "deleted.md", vaultPath: VAULT, kind: "file" as const },
        ],
        activeTab: 0,
      };
      mockTauriCommand("restore_session", () =>
        mkSession({
          pane_layout: JSON.stringify(restoredLayout),
          active_pane_id: "p1",
          tab_contents: [
            { file_path: "exists.md", vault_path: VAULT, content: "x" },
          ],
        }),
      );

      renderHook(() => useSessionPersistence());

      await waitFor(() => {
        const layout = usePaneStore.getState().layout;
        return (
          layout.type === "leaf" &&
          layout.tabs.length === 1 &&
          layout.tabs[0].filePath === "exists.md"
        );
      });
    });

    it("restores scroll_positions when present", async () => {
      const layout = { type: "leaf", id: "p1", tabs: [], activeTab: 0 };
      mockTauriCommand("restore_session", () =>
        mkSession({
          pane_layout: JSON.stringify(layout),
          active_pane_id: "p1",
          scroll_positions: JSON.stringify({ "a.md": 120 }),
        }),
      );

      renderHook(() => useSessionPersistence());

      await waitFor(() => {
        return usePaneStore.getState().scrollPositions.size === 1;
      });
      expect(usePaneStore.getState().scrollPositions.get("a.md")).toBe(120);
    });

    it("ignores invalid layout JSON", async () => {
      mockTauriCommand("restore_session", () =>
        mkSession({
          pane_layout: "not-json{",
          active_pane_id: "p1",
        }),
      );

      renderHook(() => useSessionPersistence());

      // Should not throw — wait one tick and assert default state
      await act(async () => {
        await Promise.resolve();
        await Promise.resolve();
      });
      const layout = usePaneStore.getState().layout;
      expect(layout.type).toBe("leaf");
    });

    it("recovers silently when restore_session itself fails (non-Tauri context)", async () => {
      mockTauriCommand("restore_session", () => {
        throw new Error("no tauri");
      });

      // Should not throw — hook just no-ops
      renderHook(() => useSessionPersistence());

      await act(async () => {
        await Promise.resolve();
        await Promise.resolve();
      });

      expect(useWorkspaceStore.getState().vaultPath).toBeNull();
    });

    it("does not restore vault when active_vault is null", async () => {
      mockTauriCommand("restore_session", () =>
        mkSession({ active_vault: null }),
      );

      renderHook(() => useSessionPersistence());

      await act(async () => {
        await Promise.resolve();
        await Promise.resolve();
      });

      expect(useWorkspaceStore.getState().vaultPath).toBeNull();
    });
  });

  describe("post-restore subscriptions", () => {
    it("persists vim and sidebar changes via set_config", async () => {
      mockTauriCommand("restore_session", () => mkSession());
      const calls: Array<{ key: string; value: string }> = [];
      mockTauriCommand("set_config", (args) => {
        calls.push(args as { key: string; value: string });
      });

      renderHook(() => useSessionPersistence());

      await waitFor(() => {
        expect(useWorkspaceStore.getState().vaultPath).toBe(VAULT);
      });

      // Toggle settings — should persist
      act(() => {
        useSettingsStore.getState().setVimEnabled(true);
      });
      await waitFor(() =>
        expect(calls.some((c) => c.key === "vim_enabled")).toBe(true),
      );

      act(() => {
        useSettingsStore.getState().setSidebarOpen(false);
      });
      await waitFor(() =>
        expect(calls.some((c) => c.key === "sidebar_open")).toBe(true),
      );
    });
  });
});
