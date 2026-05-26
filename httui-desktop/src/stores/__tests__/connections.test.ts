import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { useConnectionsStore } from "@/stores/connections";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";
import type { Connection } from "@/lib/tauri/connections";

const mkConn = (id: string, name: string): Connection => ({
  id,
  name,
  driver: "postgres",
  host: "localhost",
  port: 5432,
  database_name: "db",
  username: "u",
  has_password: false,
  ssl_mode: null,
  timeout_ms: 0,
  query_timeout_ms: 0,
  ttl_seconds: 0,
  max_pool_size: 0,
  is_readonly: false,
  last_tested_at: null,
  created_at: "2026-01-01T00:00:00Z",
  updated_at: "2026-01-01T00:00:00Z",
});

function resetStore() {
  useConnectionsStore.setState({ connections: [], loaded: false });
}

describe("connectionsStore", () => {
  beforeEach(() => {
    resetStore();
    clearTauriMocks();
  });
  afterEach(() => clearTauriMocks());

  describe("refresh", () => {
    it("populates connections and flips loaded", async () => {
      mockTauriCommand("list_connections", () => [mkConn("1", "pg")]);
      await useConnectionsStore.getState().refresh();
      const s = useConnectionsStore.getState();
      expect(s.connections.map((c) => c.id)).toEqual(["1"]);
      expect(s.loaded).toBe(true);
    });

    it("swallows an IPC error without throwing or mutating state", async () => {
      mockTauriCommand("list_connections", () => {
        throw new Error("ipc down");
      });
      await expect(
        useConnectionsStore.getState().refresh(),
      ).resolves.toBeUndefined();
      const s = useConnectionsStore.getState();
      expect(s.connections).toEqual([]);
      expect(s.loaded).toBe(false);
    });
  });

  describe("CRUD auto-refreshes", () => {
    it("createConnection dispatches then refreshes, returns the created row", async () => {
      const created = mkConn("9", "new");
      let createArgs: unknown = null;
      mockTauriCommand("create_connection", (a: unknown) => {
        createArgs = a;
        return created;
      });
      mockTauriCommand("list_connections", () => [created]);

      const ret = await useConnectionsStore
        .getState()
        .createConnection({ name: "new", driver: "postgres" });

      expect(ret).toEqual(created);
      expect(createArgs).toEqual({
        input: { name: "new", driver: "postgres" },
      });
      expect(useConnectionsStore.getState().connections).toEqual([created]);
    });

    it("updateConnection dispatches then refreshes", async () => {
      const updated = mkConn("3", "renamed");
      mockTauriCommand("update_connection", () => updated);
      mockTauriCommand("list_connections", () => [updated]);

      const ret = await useConnectionsStore
        .getState()
        .updateConnection("3", { name: "renamed" });

      expect(ret).toEqual(updated);
      expect(useConnectionsStore.getState().connections).toEqual([updated]);
    });

    it("deleteConnection dispatches then refreshes to the new list", async () => {
      useConnectionsStore.setState({
        connections: [mkConn("1", "a"), mkConn("2", "b")],
        loaded: true,
      });
      let deleteArgs: unknown = null;
      mockTauriCommand("delete_connection", (a: unknown) => {
        deleteArgs = a;
      });
      mockTauriCommand("list_connections", () => [mkConn("2", "b")]);

      await useConnectionsStore.getState().deleteConnection("1");

      expect(deleteArgs).toEqual({ id: "1" });
      expect(
        useConnectionsStore.getState().connections.map((c) => c.id),
      ).toEqual(["2"]);
    });
  });
});
