import { create } from "zustand";
import { devtools } from "zustand/middleware";
import { listen } from "@tauri-apps/api/event";
import {
  listWorkspace,
  setActiveVault,
  startWatching,
  stopWatching,
  rebuildSearchIndex,
} from "@/lib/tauri/commands";
import type { FileEntry } from "@/lib/tauri/commands";
import { useTagIndexStore } from "@/stores/tagIndex";

// --- Types ---

interface ConnectionStatus {
  connectionId: string;
  name: string;
  status: "connected" | "disconnected";
}

interface ConnectionStatusEvent {
  connection_id: string;
  name: string;
  status: string;
}

interface WorkspaceState {
  // Vault
  vaultPath: string | null;
  vaults: string[];
  entries: FileEntry[];

  // Connection status
  connections: Map<string, ConnectionStatus>;
  activeConnection: ConnectionStatus | null;

  // Actions
  setVaultPath: (path: string | null) => void;
  setVaults: (vaults: string[]) => void;
  setEntries: (entries: FileEntry[]) => void;
  refreshFileTree: (vault: string) => Promise<void>;
  switchVault: (path: string) => Promise<void>;
  openVault: () => Promise<void>;
}

// --- Store ---

export const useWorkspaceStore = create<WorkspaceState>()(
  devtools(
    (set, get) => ({
      vaultPath: null,
      vaults: [],
      entries: [],
      connections: new Map(),
      activeConnection: null,

      setVaultPath: (path) => set({ vaultPath: path }),
      setVaults: (vaults) => set({ vaults }),
      setEntries: (entries) => set({ entries }),

      refreshFileTree: async (vault) => {
        try {
          const tree = await listWorkspace(vault);
          set({ entries: tree });
        } catch (err) {
          console.error("Failed to list workspace:", err);
        }
      },

      switchVault: async (path) => {
        try {
          await stopWatching().catch(() => {});
          set({ vaultPath: path });
          await Promise.all([
            setActiveVault(path),
            get().refreshFileTree(path),
          ]);
          startWatching(path).catch(() => {});
          rebuildSearchIndex(path).catch(() => {});
          // Bootstrap the vault-wide tag index so quick-open #tag,
          // tag-filter dropdowns, and TagColumn autocomplete work
          // immediately after vault switch — mount.
          // Failure is non-fatal (tag index just stays at its
          // previous shape); per-save refreshes will catch up.
          useTagIndexStore
            .getState()
            .loadFromVault(path)
            .catch(() => {});
        } catch (err) {
          console.error("Failed to switch vault:", err);
        }
      },

      openVault: async () => {
        try {
          const { open: openDialog } =
            await import("@tauri-apps/plugin-dialog");
          const selected = await openDialog({
            directory: true,
            multiple: false,
          });
          if (selected) {
            await get().switchVault(selected as string);
          }
        } catch {
          const path = prompt("Enter vault path:");
          if (path) {
            await get().switchVault(path);
          }
        }
      },
    }),
    { name: "workspace-store" },
  ),
);

// --- Tauri event listeners ---

export function setupWorkspaceListeners() {
  // File watcher
  listen("fs-event", () => {
    const { vaultPath, refreshFileTree } = useWorkspaceStore.getState();
    if (vaultPath) refreshFileTree(vaultPath);
  });

  // Connection status
  listen<ConnectionStatusEvent>("connection-status", (event) => {
    const { connection_id, name, status } = event.payload;
    const { connections } = useWorkspaceStore.getState();
    const next = new Map(connections);
    if (status === "disconnected") {
      next.delete(connection_id);
    } else {
      next.set(connection_id, {
        connectionId: connection_id,
        name,
        status: status as "connected" | "disconnected",
      });
    }
    const activeConnection =
      next.size > 0 ? (Array.from(next.values()).pop() ?? null) : null;
    useWorkspaceStore.setState({ connections: next, activeConnection });
  });
}
