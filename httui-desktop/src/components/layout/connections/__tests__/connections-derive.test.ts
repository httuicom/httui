import { describe, it, expect } from "vitest";

import type { Connection } from "@/lib/tauri/connections";
import {
  buildListRows,
  countsByKind,
  envSummaries,
  isProductionName,
  listStatusCounts,
  SLOW_LATENCY_MS,
  statusFromLatency,
  type ConnectionEnrichment,
} from "@/components/layout/connections/connections-derive";

function conn(
  id: string,
  name: string,
  driver: Connection["driver"],
  host: string | null = null,
): Connection {
  return {
    id,
    name,
    driver,
    host,
    port: null,
    database_name: null,
    username: null,
    has_password: false,
    ssl_mode: null,
    timeout_ms: 0,
    query_timeout_ms: 0,
    ttl_seconds: 0,
    max_pool_size: 0,
    is_readonly: false,
    last_tested_at: null,
    created_at: "",
    updated_at: "",
  };
}

describe("isProductionName", () => {
  it("matches PROD substrings case-insensitively", () => {
    expect(isProductionName("prod-db")).toBe(true);
    expect(isProductionName("PROD")).toBe(true);
    expect(isProductionName("us-east-prod")).toBe(true);
    expect(isProductionName("production")).toBe(true);
  });

  it("does not match unrelated names containing 'pro'", () => {
    expect(isProductionName("provisioner")).toBe(false);
    expect(isProductionName("staging")).toBe(false);
  });
});

describe("statusFromLatency", () => {
  it("null → untested", () => {
    expect(statusFromLatency(null)).toBe("untested");
  });
  it("negative → down", () => {
    expect(statusFromLatency(-1)).toBe("down");
  });
  it("under threshold → ok", () => {
    expect(statusFromLatency(SLOW_LATENCY_MS - 1)).toBe("ok");
  });
  it("at threshold → slow", () => {
    expect(statusFromLatency(SLOW_LATENCY_MS)).toBe("slow");
  });
  it("over threshold → slow", () => {
    expect(statusFromLatency(SLOW_LATENCY_MS + 50)).toBe("slow");
  });
});

describe("buildListRows", () => {
  it("maps each connection through kind + enrichment", () => {
    const connections = [
      conn("a", "alpha", "postgres", "db.local"),
      conn("b", "beta", "mysql"),
      conn("c", "gamma", "sqlite"),
    ];
    const enrichment: ConnectionEnrichment[] = [
      { id: "a", env: "local", latencyMs: 12, uses: 4 },
      { id: "b", env: "prod", latencyMs: 250, uses: 20 },
    ];
    const rows = buildListRows({ connections, enrichment });
    expect(rows).toHaveLength(3);
    expect(rows[0]).toMatchObject({
      id: "a",
      kind: "postgres",
      env: "local",
      latencyMs: 12,
      status: "ok",
    });
    expect(rows[1]).toMatchObject({
      id: "b",
      kind: "mysql",
      latencyMs: 250,
      status: "slow",
    });
    expect(rows[2]).toMatchObject({
      id: "c",
      kind: "sqlite",
      env: null,
      uses: 0,
      status: "untested",
    });
  });

  it("flags PROD chip when name matches", () => {
    const rows = buildListRows({
      connections: [conn("a", "prod-db", "postgres")],
    });
    expect(rows[0].isProd).toBe(true);
  });

  it("filters by kind when kindFilter is set", () => {
    const rows = buildListRows({
      connections: [
        conn("a", "x", "postgres"),
        conn("b", "y", "mysql"),
      ],
      kindFilter: "postgres",
    });
    expect(rows).toHaveLength(1);
    expect(rows[0].kind).toBe("postgres");
  });

  it("filters out unmapped (null kind) when kindFilter is set", () => {
    const rows = buildListRows({
      connections: [conn("a", "sqlite", "sqlite")],
      kindFilter: "postgres",
    });
    expect(rows).toHaveLength(0);
  });

  it("filters by name substring (case-insensitive)", () => {
    const rows = buildListRows({
      connections: [
        conn("a", "Alpha", "postgres"),
        conn("b", "Beta", "mysql"),
      ],
      search: "alp",
    });
    expect(rows).toHaveLength(1);
    expect(rows[0].name).toBe("Alpha");
  });

  it("filters by host substring", () => {
    const rows = buildListRows({
      connections: [conn("a", "x", "postgres", "db.example.com")],
      search: "example",
    });
    expect(rows).toHaveLength(1);
  });

  it("filters by env substring (from enrichment)", () => {
    const rows = buildListRows({
      connections: [conn("a", "x", "postgres")],
      enrichment: [{ id: "a", env: "staging", latencyMs: 0, uses: 0 }],
      search: "stag",
    });
    expect(rows).toHaveLength(1);
  });

  it("treats empty search as no filter", () => {
    const rows = buildListRows({
      connections: [conn("a", "x", "postgres"), conn("b", "y", "mysql")],
      search: "   ",
    });
    expect(rows).toHaveLength(2);
  });
});

describe("countsByKind", () => {
  it("populates every canvas kind with its count + 0 for missing", () => {
    const result = countsByKind([
      conn("a", "x", "postgres"),
      conn("b", "y", "postgres"),
      conn("c", "z", "mysql"),
      conn("d", "w", "sqlite"), // unmapped — not counted
    ]);
    expect(result.postgres).toBe(2);
    expect(result.mysql).toBe(1);
    expect(result.mongo).toBe(0);
    expect(result.bigquery).toBe(0);
    expect(result.shell).toBe(0);
  });

  it("starts every kind at 0 for an empty list", () => {
    const result = countsByKind([]);
    expect(result.postgres).toBe(0);
    expect(result.mysql).toBe(0);
  });
});

describe("envSummaries", () => {
  it("aggregates counts per env name", () => {
    const summaries = envSummaries([
      { id: "a", env: "local", latencyMs: 0, uses: 0 },
      { id: "b", env: "local", latencyMs: 0, uses: 0 },
      { id: "c", env: "prod", latencyMs: 0, uses: 0 },
    ]);
    expect(summaries.find((s) => s.name === "local")?.count).toBe(2);
    expect(summaries.find((s) => s.name === "prod")?.count).toBe(1);
  });

  it("flags prod envs with warn intent", () => {
    const summaries = envSummaries([
      { id: "a", env: "prod", latencyMs: 0, uses: 0 },
      { id: "b", env: "local", latencyMs: 0, uses: 0 },
    ]);
    expect(summaries.find((s) => s.name === "prod")?.status).toBe("warn");
    expect(summaries.find((s) => s.name === "local")?.status).toBe("ok");
  });

  it("ignores rows with null env", () => {
    const summaries = envSummaries([
      { id: "a", env: null, latencyMs: 0, uses: 0 },
    ]);
    expect(summaries).toHaveLength(0);
  });

  it("returns alphabetically-sorted list", () => {
    const summaries = envSummaries([
      { id: "a", env: "zebra", latencyMs: 0, uses: 0 },
      { id: "b", env: "alpha", latencyMs: 0, uses: 0 },
      { id: "c", env: "mango", latencyMs: 0, uses: 0 },
    ]);
    expect(summaries.map((s) => s.name)).toEqual([
      "alpha",
      "mango",
      "zebra",
    ]);
  });
});

describe("listStatusCounts", () => {
  it("buckets each row by status; total = rows.length", () => {
    const rows = [
      { status: "ok" } as { status: "ok" },
      { status: "ok" } as { status: "ok" },
      { status: "slow" } as { status: "slow" },
      { status: "down" } as { status: "down" },
      { status: "untested" } as { status: "untested" },
    ];
    // Cast through unknown to satisfy ListRowItem shape minimally.
    const counts = listStatusCounts(
      rows as unknown as Parameters<typeof listStatusCounts>[0],
    );
    expect(counts.total).toBe(5);
    expect(counts.ok).toBe(2);
    expect(counts.slow).toBe(1);
    expect(counts.down).toBe(1);
  });
});
