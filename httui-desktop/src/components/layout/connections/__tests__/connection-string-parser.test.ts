import { describe, it, expect } from "vitest";

import { parseConnectionString } from "@/components/layout/connections/connection-string-parser";

describe("parseConnectionString", () => {
  it("rejects an empty string", () => {
    const result = parseConnectionString("   ");
    expect(result.ok).toBe(false);
    expect(result.ok ? "" : result.error).toMatch(/Paste/);
  });

  it("rejects a string without scheme", () => {
    const result = parseConnectionString("just-some-text");
    expect(result.ok).toBe(false);
    expect(result.ok ? "" : result.error).toMatch(/<driver>:\/\//);
  });

  it("rejects unsupported schemes", () => {
    const result = parseConnectionString("redis://localhost:6379");
    expect(result.ok).toBe(false);
    expect(result.ok ? "" : result.error).toMatch(/redis/);
  });

  it("rejects malformed URLs gracefully", () => {
    const result = parseConnectionString("postgres://[invalid");
    expect(result.ok).toBe(false);
    expect(result.ok ? "" : result.error).toMatch(/inválida/i);
  });

  it("parses a full postgres URL with sslmode + sslrootcert", () => {
    const result = parseConnectionString(
      "postgres://orders_app:hunter%402@db.internal:5432/orders?sslmode=require&sslrootcert=/etc/ca.pem",
    );
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.kind).toBe("postgres");
    expect(result.value).toMatchObject({
      name: "orders",
      host: "db.internal",
      port: "5432",
      database: "orders",
      username: "orders_app",
      password: "hunter@2",
    });
    expect(result.ssl).toMatchObject({
      mode: "require",
      rootCertPath: "/etc/ca.pem",
    });
  });

  it("normalizes postgresql:// alias to kind postgres", () => {
    const result = parseConnectionString("postgresql://localhost/orders");
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.kind).toBe("postgres");
    expect(result.value.host).toBe("localhost");
    expect(result.value.port).toBe("5432");
    expect(result.value.database).toBe("orders");
  });

  it("falls back name to host when no database is given", () => {
    const result = parseConnectionString("postgres://db.internal");
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.value.name).toBe("db.internal");
    expect(result.value.database).toBe("");
  });

  it("uses the default port for the kind when omitted", () => {
    const pg = parseConnectionString("postgres://h/db");
    expect(pg.ok && pg.value.port).toBe("5432");

    const my = parseConnectionString("mysql://h/db");
    expect(my.ok && my.value.port).toBe("3306");
  });

  it("maps mariadb scheme to mysql kind", () => {
    const result = parseConnectionString("mariadb://h:3307/db");
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.kind).toBe("mysql");
    expect(result.value.port).toBe("3307");
  });

  it("treats ssl=true as require shorthand (mysql)", () => {
    const result = parseConnectionString(
      "mysql://h/db?ssl=true&ssl-ca=/etc/ca.pem&ssl-cert=/c.pem&ssl-key=/k.pem",
    );
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.ssl).toMatchObject({
      mode: "require",
      rootCertPath: "/etc/ca.pem",
      clientCertPath: "/c.pem",
      clientKeyPath: "/k.pem",
    });
  });

  it("ignores unrecognised sslmode values", () => {
    const result = parseConnectionString("postgres://h/db?sslmode=bogus");
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.ssl.mode).toBe("");
  });

  it("decodes percent-encoded credentials", () => {
    const result = parseConnectionString("postgres://us%40er:pa%2Fss@h/db");
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.value.username).toBe("us@er");
    expect(result.value.password).toBe("pa/ss");
  });
});
