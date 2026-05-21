import { describe, it, expect, vi } from "vitest";

// Mock the cm-*-block extension factories so the registry can be loaded
// without pulling in CM6 / Tauri trees. The mocks return distinguishable
// sentinels so we can assert the registry wires them correctly.
vi.mock("@/lib/codemirror/cm-db-block", () => ({
  createDbBlockExtension: vi.fn(() => ({ dbExt: true })),
  createDbBlockCompletionSource: vi.fn(
    (getFilePath: () => string | undefined) => ({
      dbRefSource: true,
      getFilePath,
    }),
  ),
  createDbSchemaCompletionSource: vi.fn(() => ({ dbSchemaSource: true })),
}));
vi.mock("@/lib/codemirror/cm-http-block", () => ({
  createHttpBlockExtension: vi.fn(() => ({ httpExt: true })),
  createHttpBlockCompletionSource: vi.fn(
    (getFilePath: () => string | undefined) => ({
      httpRefSource: true,
      getFilePath,
    }),
  ),
}));

import {
  blockRegistry,
  getRegisteredBlockIcons,
  getRegisteredBlockSlashCommands,
} from "@/lib/blocks/block-registry";
import type { CompletionSection } from "@codemirror/autocomplete";

describe("blockRegistry", () => {
  it("registers exactly two block types in DB-then-HTTP order", () => {
    expect(blockRegistry.map((m) => m.id)).toEqual(["db", "http"]);
  });

  it("each entry carries a label + icons + slashCommands + factories", () => {
    for (const m of blockRegistry) {
      expect(typeof m.label).toBe("string");
      expect(m.label.length).toBeGreaterThan(0);
      expect(Array.isArray(m.icons)).toBe(true);
      expect(m.icons.length).toBeGreaterThan(0);
      expect(Array.isArray(m.slashCommands)).toBe(true);
      expect(m.slashCommands.length).toBeGreaterThan(0);
      expect(typeof m.createExtension).toBe("function");
      expect(typeof m.completionSources).toBe("function");
    }
  });

  it("DB module supplies the database icon + 3 slash entries", () => {
    const db = blockRegistry.find((m) => m.id === "db")!;
    expect(db.icons.map((i) => i.type)).toEqual(["database"]);
    expect(db.icons[0].paths).toContain("ellipse");
    expect(db.slashCommands.map((c) => c.label)).toEqual([
      "PostgreSQL Query",
      "MySQL Query",
      "SQLite Query",
    ]);
    expect(db.slashCommands[0].insert).toContain("db-postgres");
    expect(db.slashCommands[0].cursorOffset).toBe(-5);
  });

  it("HTTP module supplies the http icon + 5 slash entries", () => {
    const http = blockRegistry.find((m) => m.id === "http")!;
    expect(http.icons.map((i) => i.type)).toEqual(["http"]);
    expect(http.slashCommands.map((c) => c.label)).toEqual([
      "HTTP Request",
      "HTTP GET",
      "HTTP POST",
      "HTTP PUT",
      "HTTP DELETE",
    ]);
    // POST/PUT insert templates carry the Content-Type header.
    const post = http.slashCommands.find((c) => c.label === "HTTP POST");
    expect(post?.insert).toContain("Content-Type: application/json");
    expect(post?.cursorOffset).toBe(-23);
  });

  it("createExtension delegates to the underlying cm-*-block factory", () => {
    const dbExt = blockRegistry.find((m) => m.id === "db")!.createExtension();
    const httpExt = blockRegistry
      .find((m) => m.id === "http")!
      .createExtension();
    expect(dbExt).toEqual({ dbExt: true });
    expect(httpExt).toEqual({ httpExt: true });
  });

  it("DB completionSources returns [ref, schema] in that order", () => {
    const getFilePath = () => "current.md";
    const sources = blockRegistry
      .find((m) => m.id === "db")!
      .completionSources(getFilePath);
    expect(sources).toHaveLength(2);
    expect(sources[0]).toMatchObject({ dbRefSource: true });
    expect(sources[1]).toMatchObject({ dbSchemaSource: true });
    // The getFilePath getter is forwarded to the ref source.
    expect(
      (sources[0] as unknown as { getFilePath: () => string }).getFilePath(),
    ).toBe("current.md");
  });

  it("HTTP completionSources returns only the ref source", () => {
    const getFilePath = () => "current.md";
    const sources = blockRegistry
      .find((m) => m.id === "http")!
      .completionSources(getFilePath);
    expect(sources).toHaveLength(1);
    expect(sources[0]).toMatchObject({ httpRefSource: true });
  });
});

describe("getRegisteredBlockIcons", () => {
  it("returns a Record<type, paths> keyed by icon type", () => {
    const icons = getRegisteredBlockIcons();
    expect(icons).toHaveProperty("database");
    expect(icons).toHaveProperty("http");
    expect(icons.database).toContain("ellipse");
    expect(icons.http).toContain("path d=");
  });

  it("includes every icon declared by every block module", () => {
    const expectedTypes = blockRegistry.flatMap((m) =>
      m.icons.map((i) => i.type),
    );
    const got = Object.keys(getRegisteredBlockIcons());
    for (const t of expectedTypes) {
      expect(got).toContain(t);
    }
  });
});

describe("getRegisteredBlockSlashCommands", () => {
  const EXEC: CompletionSection = { name: "Executable", rank: 2 };

  it("returns the flat slash commands list with the section injected", () => {
    const out = getRegisteredBlockSlashCommands(EXEC);
    // 3 (DB) + 5 (HTTP) = 8 entries.
    expect(out).toHaveLength(8);
    for (const entry of out) {
      expect(entry.section).toBe(EXEC);
    }
  });

  it("preserves the per-module ordering (DB entries before HTTP)", () => {
    const labels = getRegisteredBlockSlashCommands(EXEC).map((s) => s.label);
    expect(labels.slice(0, 3)).toEqual([
      "PostgreSQL Query",
      "MySQL Query",
      "SQLite Query",
    ]);
    expect(labels.slice(3)).toEqual([
      "HTTP Request",
      "HTTP GET",
      "HTTP POST",
      "HTTP PUT",
      "HTTP DELETE",
    ]);
  });

  it("forwards insert + cursorOffset from the underlying spec", () => {
    const out = getRegisteredBlockSlashCommands(EXEC);
    const pg = out.find((s) => s.label === "PostgreSQL Query")!;
    expect(pg.insert).toContain("db-postgres");
    expect(pg.cursorOffset).toBe(-5);
    expect(pg.type).toBe("database");
  });
});
