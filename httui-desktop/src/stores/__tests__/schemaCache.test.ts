import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { useSchemaCacheStore } from "@/stores/schemaCache";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";
import type { SchemaEntry } from "@/lib/tauri/connections";

const CONN = "conn-1";

const mkEntry = (
  schema: string | null,
  table: string,
  column: string,
  dataType: string | null = "text",
): SchemaEntry => ({
  schema_name: schema,
  table_name: table,
  column_name: column,
  data_type: dataType,
});

function resetStore() {
  useSchemaCacheStore.setState({ byConnection: {} });
}

describe("schemaCacheStore", () => {
  beforeEach(() => {
    resetStore();
    clearTauriMocks();
  });

  afterEach(() => {
    clearTauriMocks();
  });

  describe("get (sync)", () => {
    it("returns null when nothing is cached", () => {
      expect(useSchemaCacheStore.getState().get(CONN)).toBeNull();
    });

    it("returns cached schema after ensureLoaded", async () => {
      mockTauriCommand("get_cached_schema", () => [
        mkEntry("public", "users", "id"),
      ]);
      mockTauriCommand("introspect_schema", () => []);

      await useSchemaCacheStore.getState().ensureLoaded(CONN);

      const cached = useSchemaCacheStore.getState().get(CONN);
      expect(cached).not.toBeNull();
      expect(cached?.tables).toHaveLength(1);
    });
  });

  describe("ensureLoaded — dual path", () => {
    it("uses SQLite cache when present (no introspect call)", async () => {
      let introspectCalls = 0;
      mockTauriCommand("get_cached_schema", () => [
        mkEntry("public", "t1", "c1"),
      ]);
      mockTauriCommand("introspect_schema", () => {
        introspectCalls++;
        return [];
      });

      const result = await useSchemaCacheStore.getState().ensureLoaded(CONN);

      expect(introspectCalls).toBe(0);
      expect(result?.tables[0].name).toBe("t1");
    });

    it("falls back to introspect when cache returns empty", async () => {
      mockTauriCommand("get_cached_schema", () => []);
      mockTauriCommand("introspect_schema", () => [
        mkEntry("public", "fresh", "id"),
      ]);

      const result = await useSchemaCacheStore.getState().ensureLoaded(CONN);

      expect(result?.tables[0].name).toBe("fresh");
    });

    it("dedups parallel calls (single inflight promise per connection)", async () => {
      let introspectCalls = 0;
      mockTauriCommand("get_cached_schema", () => []);
      mockTauriCommand("introspect_schema", () => {
        introspectCalls++;
        return [mkEntry("public", "t1", "c1")];
      });

      // Fire two parallel calls before resolving
      const [a, b] = await Promise.all([
        useSchemaCacheStore.getState().ensureLoaded(CONN),
        useSchemaCacheStore.getState().ensureLoaded(CONN),
      ]);

      expect(introspectCalls).toBe(1);
      expect(a).toEqual(b);
    });

    it("returns cached entry on subsequent calls without re-fetching", async () => {
      let cacheCalls = 0;
      mockTauriCommand("get_cached_schema", () => {
        cacheCalls++;
        return [mkEntry("public", "t1", "c1")];
      });
      mockTauriCommand("introspect_schema", () => []);

      await useSchemaCacheStore.getState().ensureLoaded(CONN);
      await useSchemaCacheStore.getState().ensureLoaded(CONN);
      await useSchemaCacheStore.getState().ensureLoaded(CONN);

      expect(cacheCalls).toBe(1);
    });

    it("captures error message on failure", async () => {
      mockTauriCommand("get_cached_schema", () => {
        throw new Error("connection refused");
      });

      const result = await useSchemaCacheStore.getState().ensureLoaded(CONN);

      expect(result).toBeNull();
      const entry = useSchemaCacheStore.getState().byConnection[CONN];
      expect(entry?.error).toBe("connection refused");
      expect(entry?.loading).toBe(false);
      expect(entry?.inflight).toBeNull();
    });
  });

  describe("refresh", () => {
    it("forces introspect, bypassing cache", async () => {
      let cacheCalls = 0;
      mockTauriCommand("get_cached_schema", () => {
        cacheCalls++;
        return [mkEntry("public", "stale", "c")];
      });
      mockTauriCommand("introspect_schema", () => [
        mkEntry("public", "fresh", "c"),
      ]);

      const result = await useSchemaCacheStore.getState().refresh(CONN);

      expect(cacheCalls).toBe(0);
      expect(result?.tables[0].name).toBe("fresh");
    });

    it("dedups parallel refresh calls", async () => {
      let introspectCalls = 0;
      mockTauriCommand("introspect_schema", () => {
        introspectCalls++;
        return [];
      });

      await Promise.all([
        useSchemaCacheStore.getState().refresh(CONN),
        useSchemaCacheStore.getState().refresh(CONN),
      ]);

      expect(introspectCalls).toBe(1);
    });

    it("captures error on refresh failure", async () => {
      mockTauriCommand("introspect_schema", () => {
        throw new Error("no driver");
      });

      const result = await useSchemaCacheStore.getState().refresh(CONN);

      expect(result).toBeNull();
      expect(useSchemaCacheStore.getState().byConnection[CONN]?.error).toBe(
        "no driver",
      );
    });

    it("handles non-Error throw values", async () => {
      mockTauriCommand("introspect_schema", () => {
        throw "string error";
      });

      const result = await useSchemaCacheStore.getState().refresh(CONN);

      expect(result).toBeNull();
      expect(useSchemaCacheStore.getState().byConnection[CONN]?.error).toBe(
        "string error",
      );
    });
  });

  describe("invalidate", () => {
    it("removes the connection entry", async () => {
      mockTauriCommand("get_cached_schema", () => [
        mkEntry("public", "t", "c"),
      ]);
      mockTauriCommand("introspect_schema", () => []);

      await useSchemaCacheStore.getState().ensureLoaded(CONN);
      expect(useSchemaCacheStore.getState().byConnection[CONN]).toBeDefined();

      useSchemaCacheStore.getState().invalidate(CONN);

      expect(useSchemaCacheStore.getState().byConnection[CONN]).toBeUndefined();
    });

    it("is a no-op for unknown connection", () => {
      expect(() =>
        useSchemaCacheStore.getState().invalidate("never-loaded"),
      ).not.toThrow();
    });
  });

  describe("table grouping", () => {
    it("groups columns under their parent table", async () => {
      mockTauriCommand("get_cached_schema", () => [
        mkEntry("public", "users", "id", "int"),
        mkEntry("public", "users", "name", "text"),
        mkEntry("public", "users", "email", "text"),
      ]);
      mockTauriCommand("introspect_schema", () => []);

      const result = await useSchemaCacheStore.getState().ensureLoaded(CONN);

      expect(result?.tables).toHaveLength(1);
      expect(result?.tables[0].columns).toEqual([
        { name: "id", dataType: "int" },
        { name: "name", dataType: "text" },
        { name: "email", dataType: "text" },
      ]);
    });

    it("differentiates same-named tables in different schemas", async () => {
      mockTauriCommand("get_cached_schema", () => [
        mkEntry("public", "users", "id"),
        mkEntry("auth", "users", "session_id"),
      ]);
      mockTauriCommand("introspect_schema", () => []);

      const result = await useSchemaCacheStore.getState().ensureLoaded(CONN);

      expect(result?.tables).toHaveLength(2);
      const schemas = result?.tables.map((t) => t.schema);
      expect(schemas).toContain("public");
      expect(schemas).toContain("auth");
    });

    it("sorts tables by schema then name", async () => {
      mockTauriCommand("get_cached_schema", () => [
        mkEntry("public", "z", "c"),
        mkEntry("auth", "b", "c"),
        mkEntry("public", "a", "c"),
        mkEntry("auth", "a", "c"),
      ]);
      mockTauriCommand("introspect_schema", () => []);

      const result = await useSchemaCacheStore.getState().ensureLoaded(CONN);

      expect(result?.tables.map((t) => `${t.schema}.${t.name}`)).toEqual([
        "auth.a",
        "auth.b",
        "public.a",
        "public.z",
      ]);
    });

    it("handles SQLite-style null schema", async () => {
      mockTauriCommand("get_cached_schema", () => [
        mkEntry(null, "todos", "id"),
      ]);
      mockTauriCommand("introspect_schema", () => []);

      const result = await useSchemaCacheStore.getState().ensureLoaded(CONN);

      expect(result?.tables[0].schema).toBeNull();
    });
  });
});
