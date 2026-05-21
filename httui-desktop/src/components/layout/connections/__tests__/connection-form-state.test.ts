import { describe, it, expect } from "vitest";

import type { Connection } from "@/lib/tauri/connections";
import {
  initConnectionFormState,
  emptyConnectionFormState,
  connectionFormReducer,
  validateConnection,
  buildConnectionInput,
  type ConnectionFormState,
} from "@/components/layout/connections/connection-form-state";

const mkConnection = (over: Partial<Connection> = {}): Connection => ({
  id: "c1",
  name: "primary",
  driver: "postgres",
  host: "db.test",
  port: 6543,
  database_name: "mydb",
  username: "alice",
  has_password: false,
  ssl_mode: "require",
  timeout_ms: 9000,
  query_timeout_ms: 25000,
  ttl_seconds: 120,
  max_pool_size: 8,
  is_readonly: false,
  last_tested_at: null,
  created_at: "",
  updated_at: "",
  ...over,
});

const base = (): ConnectionFormState => initConnectionFormState(null);

describe("initConnectionFormState", () => {
  it("seeds a new connection from driver defaults (port from DRIVER_CONFIG)", () => {
    const s = initConnectionFormState(null);
    expect(s.name).toBe("");
    expect(s.driver).toBe("postgres");
    expect(s.host).toBe("localhost");
    expect(s.port).toBe("5432"); // postgres defaultPort — replaces the old mount effect
    expect(s.password).toBe("");
    expect(s.saving).toBe(false);
    expect(s.error).toBeNull();
    expect(s.timeoutMs).toBe("10000");
  });

  it("prefills + keeps the stored port in edit mode", () => {
    const s = initConnectionFormState(mkConnection());
    expect(s.name).toBe("primary");
    expect(s.host).toBe("db.test");
    expect(s.port).toBe("6543"); // stored port kept (old effect was gated on !isEdit)
    expect(s.username).toBe("alice");
    expect(s.sslMode).toBe("require");
    expect(s.timeoutMs).toBe("9000");
    expect(s.password).toBe(""); // never echoed back
  });
});

describe("connectionFormReducer", () => {
  it("setField updates only the keyed field", () => {
    const s = connectionFormReducer(base(), {
      type: "setField",
      field: "name",
      value: "staging",
    });
    expect(s.name).toBe("staging");
    expect(s.host).toBe("localhost");
  });

  it("setDriver re-derives the default port for a NEW connection", () => {
    const mysql = connectionFormReducer(base(), {
      type: "setDriver",
      driver: "mysql",
      isEdit: false,
    });
    expect(mysql.driver).toBe("mysql");
    expect(mysql.port).toBe("3306");
    const sqlite = connectionFormReducer(base(), {
      type: "setDriver",
      driver: "sqlite",
      isEdit: false,
    });
    expect(sqlite.port).toBe("");
  });

  it("setDriver keeps the stored port when editing", () => {
    const edit = initConnectionFormState(mkConnection()); // port "6543"
    const next = connectionFormReducer(edit, {
      type: "setDriver",
      driver: "mysql",
      isEdit: true,
    });
    expect(next.driver).toBe("mysql");
    expect(next.port).toBe("6543");
  });

  it("toggleAdvanced flips the flag", () => {
    const open = connectionFormReducer(base(), { type: "toggleAdvanced" });
    expect(open.showAdvanced).toBe(true);
    expect(
      connectionFormReducer(open, { type: "toggleAdvanced" }).showAdvanced,
    ).toBe(false);
  });

  it("save lifecycle: start clears error, error sets it, done clears saving", () => {
    const start = connectionFormReducer(
      { ...base(), error: "old" },
      { type: "saveStart" },
    );
    expect(start).toMatchObject({ saving: true, error: null });
    const err = connectionFormReducer(start, {
      type: "saveError",
      message: "boom",
    });
    expect(err).toMatchObject({ saving: false, error: "boom" });
    expect(connectionFormReducer(start, { type: "saveDone" }).saving).toBe(
      false,
    );
  });

  it("test lifecycle: start resets, success + failure set the result", () => {
    const start = connectionFormReducer(base(), { type: "testStart" });
    expect(start).toMatchObject({
      testing: true,
      testResult: null,
      testError: null,
    });
    expect(connectionFormReducer(start, { type: "testSuccess" })).toMatchObject(
      { testing: false, testResult: "success" },
    );
    expect(
      connectionFormReducer(start, {
        type: "testFailure",
        message: "DNS",
      }),
    ).toMatchObject({
      testing: false,
      testResult: "error",
      testError: "DNS",
    });
  });
});

describe("validateConnection", () => {
  it("requires a name", () => {
    const r = validateConnection({ ...base(), name: "  " });
    expect(r).toEqual({ ok: false, reason: "Connection name is required" });
  });

  it("sqlite needs a file path, then passes", () => {
    const empty = validateConnection({
      ...base(),
      name: "db",
      driver: "sqlite",
      dbName: "",
    });
    expect(empty).toEqual({
      ok: false,
      reason: "SQLite file path is required",
    });
    expect(
      validateConnection({
        ...base(),
        name: "db",
        driver: "sqlite",
        dbName: "/tmp/x.db",
      }),
    ).toEqual({ ok: true });
  });

  it("network drivers require a host", () => {
    expect(validateConnection({ ...base(), name: "db", host: "" })).toEqual({
      ok: false,
      reason: "Host is required",
    });
  });

  it.each(["", "abc", "0", "-1", "70000", "12.5"])(
    "rejects a bad port %j",
    (port) => {
      const r = validateConnection({ ...base(), name: "db", port });
      expect(r.ok).toBe(false);
      if (!r.ok) expect(r.reason).toMatch(/port/i);
    },
  );

  it("passes a well-formed network connection", () => {
    expect(
      validateConnection({
        ...base(),
        name: "db",
        host: "localhost",
        port: "5432",
      }),
    ).toEqual({ ok: true });
  });
});

describe("emptyConnectionFormState", () => {
  it("is exactly what initConnectionFormState(null) returns", () => {
    expect(initConnectionFormState(null)).toEqual(emptyConnectionFormState());
  });
});

describe("buildConnectionInput", () => {
  it("includes host/port/credentials/ssl for a network driver", () => {
    const input = buildConnectionInput({
      ...base(),
      name: "pg",
      host: "db.x",
      port: "5432",
      username: "u",
      password: "p",
      sslMode: "require",
      dbName: "app",
    });
    expect(input).toMatchObject({
      name: "pg",
      driver: "postgres",
      host: "db.x",
      port: 5432,
      username: "u",
      password: "p",
      ssl_mode: "require",
      database_name: "app",
    });
  });

  it("omits network fields for sqlite (only db path + advanced)", () => {
    const input = buildConnectionInput({
      ...base(),
      name: "local",
      driver: "sqlite",
      dbName: "/tmp/a.db",
    });
    expect(input).toMatchObject({
      name: "local",
      driver: "sqlite",
      database_name: "/tmp/a.db",
    });
    expect(input).not.toHaveProperty("host");
    expect(input).not.toHaveProperty("port");
    expect(input).not.toHaveProperty("username");
    expect(input).not.toHaveProperty("ssl_mode");
  });

  it("coerces blank/garbage numerics to undefined (lenient, as before)", () => {
    const input = buildConnectionInput({
      ...base(),
      name: "pg",
      port: "abc",
      timeoutMs: "",
      maxPoolSize: "x",
    });
    expect(input.port).toBeUndefined();
    expect(input.timeout_ms).toBeUndefined();
    expect(input.max_pool_size).toBeUndefined();
  });
});
