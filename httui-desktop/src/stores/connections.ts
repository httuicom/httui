import { create } from "zustand";
import { devtools } from "zustand/middleware";
import {
  listConnections,
  createConnection as createConnectionCmd,
  updateConnection as updateConnectionCmd,
  deleteConnection as deleteConnectionCmd,
  type Connection,
  type CreateConnectionInput,
  type UpdateConnectionInput,
} from "@/lib/tauri/connections";

/**
 * Single source of truth for the connection list. Mirrors
 * `environment.ts`: `refresh()` pulls the IPC list; CRUD actions
 * dispatch the command then `refresh()` so every surface (the
 * ConnectionsPage detail panel and the sidebar ConnectionsList)
 * stays in sync without per-component `listConnections()` copies or
 * manual `reload()` fan-out.
 *
 * The `config-changed` file-watcher subscription is intentionally NOT
 * owned here (mirrors environment.ts — it stays a consumer concern,
 * wired via `useConfigSyncedResource`), keeping the store pure
 * data + actions.
 */
interface ConnectionsState {
  connections: Connection[];
  /** False until the first successful `refresh()` — lets consumers
   *  distinguish "not loaded yet" from "loaded, empty". */
  loaded: boolean;

  refresh: () => Promise<void>;
  createConnection: (input: CreateConnectionInput) => Promise<Connection>;
  updateConnection: (
    id: string,
    input: UpdateConnectionInput,
  ) => Promise<Connection>;
  deleteConnection: (id: string) => Promise<void>;
}

export const useConnectionsStore = create<ConnectionsState>()(
  devtools(
    (set, get) => ({
      connections: [],
      loaded: false,

      refresh: async () => {
        try {
          const list = await listConnections();
          set({ connections: list, loaded: true });
        } catch {
          /* silently fail — mirrors environment.ts */
        }
      },

      createConnection: async (input) => {
        const created = await createConnectionCmd(input);
        await get().refresh();
        return created;
      },

      updateConnection: async (id, input) => {
        const updated = await updateConnectionCmd(id, input);
        await get().refresh();
        return updated;
      },

      deleteConnection: async (id) => {
        await deleteConnectionCmd(id);
        await get().refresh();
      },
    }),
    { name: "connections-store" },
  ),
);
