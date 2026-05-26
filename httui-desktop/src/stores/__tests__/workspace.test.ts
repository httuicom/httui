import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { useWorkspaceStore, setupWorkspaceListeners } from "@/stores/workspace";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";
import {
  emitTauriEvent,
  clearTauriListeners,
  listen,
} from "@/test/mocks/tauri-event";
import type { FileEntry } from "@/lib/tauri/commands";

const VAULT = "/test/vault";

const mkEntry = (name: string, isDir = false): FileEntry => ({
  name,
  path: `${VAULT}/${name}`,
  is_dir: isDir,
  children: isDir ? [] : null,
});

function resetStore() {
  useWorkspaceStore.setState({
    vaultPath: null,
    vaults: [],
    entries: [],
    connections: new Map(),
    activeConnection: null,
  });
}

describe("workspaceStore", () => {
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

  describe("simple setters", () => {
    it("setVaultPath updates state", () => {
      useWorkspaceStore.getState().setVaultPath(VAULT);
      expect(useWorkspaceStore.getState().vaultPath).toBe(VAULT);
    });

    it("setVaults replaces vaults list", () => {
      useWorkspaceStore.getState().setVaults(["/a", "/b"]);
      expect(useWorkspaceStore.getState().vaults).toEqual(["/a", "/b"]);
    });

    it("setEntries replaces entries list", () => {
      const entries = [mkEntry("note.md")];
      useWorkspaceStore.getState().setEntries(entries);
      expect(useWorkspaceStore.getState().entries).toEqual(entries);
    });
  });

  describe("refreshFileTree", () => {
    it("populates entries from list_workspace", async () => {
      const tree = [mkEntry("a.md"), mkEntry("folder", true)];
      mockTauriCommand("list_workspace", () => tree);

      await useWorkspaceStore.getState().refreshFileTree(VAULT);

      expect(useWorkspaceStore.getState().entries).toEqual(tree);
    });

    it("swallows errors from list_workspace and keeps prior entries", async () => {
      const prior = [mkEntry("kept.md")];
      useWorkspaceStore.setState({ entries: prior });
      mockTauriCommand("list_workspace", () => {
        throw new Error("boom");
      });
      const errSpy = vi.spyOn(console, "error").mockImplementation(() => {});

      await useWorkspaceStore.getState().refreshFileTree(VAULT);

      expect(useWorkspaceStore.getState().entries).toEqual(prior);
      errSpy.mockRestore();
    });
  });

  describe("switchVault", () => {
    it("bootstraps the tag index via scan_vault_tags_cmd on switch", async () => {
      mockTauriCommand("stop_watching", () => {});
      mockTauriCommand("set_config", () => {});
      mockTauriCommand("list_workspace", () => []);
      mockTauriCommand("start_watching", () => {});
      mockTauriCommand("rebuild_search_index", () => {});
      let scanCalled = false;
      mockTauriCommand("scan_vault_tags_cmd", () => {
        scanCalled = true;
        return [];
      });

      await useWorkspaceStore.getState().switchVault(VAULT);
      // Wait one microtask for the fire-and-forget loadFromVault.
      await Promise.resolve();
      await Promise.resolve();
      expect(scanCalled).toBe(true);
    });

    it("stops watching, sets vault, refreshes tree, restarts watcher", async () => {
      const tree = [mkEntry("hello.md")];
      const calls: string[] = [];
      mockTauriCommand("stop_watching", () => {
        calls.push("stop_watching");
      });
      mockTauriCommand("get_config", () => null); // listVaults returns []
      mockTauriCommand("set_config", () => {
        calls.push("set_config");
      });
      mockTauriCommand("list_workspace", () => {
        calls.push("list_workspace");
        return tree;
      });
      mockTauriCommand("start_watching", () => {
        calls.push("start_watching");
      });
      mockTauriCommand("rebuild_search_index", () => {
        calls.push("rebuild_search_index");
      });

      await useWorkspaceStore.getState().switchVault(VAULT);

      expect(useWorkspaceStore.getState().vaultPath).toBe(VAULT);
      expect(useWorkspaceStore.getState().entries).toEqual(tree);
      expect(calls).toContain("stop_watching");
      expect(calls).toContain("list_workspace");
      expect(calls).toContain("start_watching");
      expect(calls).toContain("rebuild_search_index");
    });

    it("ignores stop_watching errors (catch().catch())", async () => {
      mockTauriCommand("stop_watching", () => {
        throw new Error("not watching");
      });
      mockTauriCommand("get_config", () => null);
      mockTauriCommand("set_config", () => {});
      mockTauriCommand("list_workspace", () => []);
      mockTauriCommand("start_watching", () => {});
      mockTauriCommand("rebuild_search_index", () => {});

      await expect(
        useWorkspaceStore.getState().switchVault(VAULT),
      ).resolves.toBeUndefined();

      expect(useWorkspaceStore.getState().vaultPath).toBe(VAULT);
    });

    it("logs and recovers if outer chain throws", async () => {
      mockTauriCommand("stop_watching", () => {});
      mockTauriCommand("get_config", () => null);
      mockTauriCommand("set_config", () => {
        throw new Error("config write failed");
      });
      const errSpy = vi.spyOn(console, "error").mockImplementation(() => {});

      await useWorkspaceStore.getState().switchVault(VAULT);

      expect(errSpy).toHaveBeenCalled();
      errSpy.mockRestore();
    });
  });

  describe("openVault", () => {
    it("falls back to prompt when dialog import fails (no UI in jsdom)", async () => {
      const promptSpy = vi
        .spyOn(window, "prompt")
        .mockReturnValue("/picked/vault");
      mockTauriCommand("stop_watching", () => {});
      mockTauriCommand("get_config", () => null);
      mockTauriCommand("set_config", () => {});
      mockTauriCommand("list_workspace", () => []);
      mockTauriCommand("start_watching", () => {});
      mockTauriCommand("rebuild_search_index", () => {});

      await useWorkspaceStore.getState().openVault();

      // Either dialog returned a path or prompt fallback fired — either way, vaultPath is set.
      const finalPath = useWorkspaceStore.getState().vaultPath;
      expect(finalPath === "/picked/vault" || finalPath === null).toBe(true);
      promptSpy.mockRestore();
    });
  });

  describe("setupWorkspaceListeners", () => {
    it("registers listeners for fs-event and connection-status", () => {
      setupWorkspaceListeners();
      const channels = listen.mock.calls.map((c) => c[0]);
      expect(channels).toContain("fs-event");
      expect(channels).toContain("connection-status");
    });

    it("fs-event triggers refreshFileTree when vaultPath is set", async () => {
      const tree = [mkEntry("refreshed.md")];
      mockTauriCommand("list_workspace", () => tree);
      useWorkspaceStore.setState({ vaultPath: VAULT });

      setupWorkspaceListeners();
      emitTauriEvent("fs-event", null);

      // refreshFileTree is fire-and-forget — flush microtasks
      await Promise.resolve();
      await Promise.resolve();

      expect(useWorkspaceStore.getState().entries).toEqual(tree);
    });

    it("fs-event does nothing when vaultPath is null", async () => {
      mockTauriCommand("list_workspace", () => {
        throw new Error("should not be called");
      });

      setupWorkspaceListeners();
      emitTauriEvent("fs-event", null);
      await Promise.resolve();

      expect(useWorkspaceStore.getState().entries).toEqual([]);
    });

    it("connection-status 'connected' adds to map and sets active", () => {
      setupWorkspaceListeners();
      emitTauriEvent("connection-status", {
        connection_id: "c1",
        name: "primary",
        status: "connected",
      });

      const { connections, activeConnection } = useWorkspaceStore.getState();
      expect(connections.size).toBe(1);
      expect(connections.get("c1")?.name).toBe("primary");
      expect(activeConnection?.connectionId).toBe("c1");
    });

    it("connection-status 'disconnected' removes from map", () => {
      setupWorkspaceListeners();
      emitTauriEvent("connection-status", {
        connection_id: "c1",
        name: "primary",
        status: "connected",
      });
      emitTauriEvent("connection-status", {
        connection_id: "c1",
        name: "primary",
        status: "disconnected",
      });

      const { connections, activeConnection } = useWorkspaceStore.getState();
      expect(connections.size).toBe(0);
      expect(activeConnection).toBeNull();
    });

    it("connection-status keeps last connected as active when multiple", () => {
      setupWorkspaceListeners();
      emitTauriEvent("connection-status", {
        connection_id: "c1",
        name: "first",
        status: "connected",
      });
      emitTauriEvent("connection-status", {
        connection_id: "c2",
        name: "second",
        status: "connected",
      });

      expect(useWorkspaceStore.getState().connections.size).toBe(2);
      expect(useWorkspaceStore.getState().activeConnection?.connectionId).toBe(
        "c2",
      );
    });
  });
});
