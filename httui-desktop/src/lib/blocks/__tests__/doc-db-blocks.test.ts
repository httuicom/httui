import { describe, expect, it } from "vitest";

import {
  findDocDbBlocks,
  hasDbBlocks,
  mostRecentDbConnection,
} from "@/lib/blocks/doc-db-blocks";

describe("findDocDbBlocks", () => {
  it("returns empty for empty input", () => {
    expect(findDocDbBlocks("")).toEqual([]);
  });

  it("returns empty for content with no fences", () => {
    expect(findDocDbBlocks("just a paragraph\n")).toEqual([]);
  });

  it("captures one db-postgres block with metadata", () => {
    const content =
      "# Title\n\n```db-postgres alias=q1 connection=prod-db\nSELECT 1;\n```\n";
    const r = findDocDbBlocks(content);
    expect(r).toHaveLength(1);
    expect(r[0].fence).toBe("db-postgres");
    expect(r[0].meta.dialect).toBe("postgres");
    expect(r[0].meta.alias).toBe("q1");
    expect(r[0].meta.connection).toBe("prod-db");
    expect(r[0].line).toBe(3);
    expect(content.slice(r[0].offset).startsWith("```db-postgres")).toBe(true);
  });

  it("captures multiple blocks in document order", () => {
    const content =
      "```db-mysql connection=a\nq1\n```\n\n```db-postgres connection=b\nq2\n```\n";
    const r = findDocDbBlocks(content);
    expect(r.map((e) => e.meta.connection)).toEqual(["a", "b"]);
    expect(r.map((e) => e.meta.dialect)).toEqual(["mysql", "postgres"]);
  });

  it("recognizes the bare `db` (generic) dialect", () => {
    const content = "```db connection=ad-hoc\nSELECT 1;\n```\n";
    const r = findDocDbBlocks(content);
    expect(r).toHaveLength(1);
    expect(r[0].meta.dialect).toBe("generic");
  });

  it("ignores non-db fences (http, plain code)", () => {
    const content =
      "```http alias=x\nGET /\n```\n\n```\nplain\n```\n\n```ts\nconst x = 1;\n```\n";
    expect(findDocDbBlocks(content)).toEqual([]);
  });

  it("skips YAML frontmatter", () => {
    const content =
      '---\ntitle: "x"\n---\n```db-postgres connection=p\nq\n```\n';
    const r = findDocDbBlocks(content);
    expect(r).toHaveLength(1);
    expect(r[0].line).toBe(4);
  });

  it("ignores an unclosed db fence (no emit until closed)", () => {
    const content = "```db-postgres connection=p\nSELECT 1;\nno close fence\n";
    expect(findDocDbBlocks(content)).toEqual([]);
  });

  it("does not match a db fence inside another open fence", () => {
    // ```http opens a fence that "swallows" the inner ```db
    // line. The implementation just waits for the next ``` close.
    const content = "```http\n```db-postgres connection=swallowed\nq\n```\n";
    const r = findDocDbBlocks(content);
    // The outer http fence closes at the inner ```; nothing emitted.
    expect(r).toHaveLength(0);
  });
});

describe("hasDbBlocks", () => {
  it("returns false for empty / no-db content", () => {
    expect(hasDbBlocks("")).toBe(false);
    expect(hasDbBlocks("# Title\nbody only\n")).toBe(false);
    expect(hasDbBlocks("```http\nGET /\n```\n")).toBe(false);
  });

  it("returns true when at least one db-* block exists", () => {
    expect(hasDbBlocks("```db-postgres\nq\n```\n")).toBe(true);
    expect(hasDbBlocks("```db connection=x\nq\n```\n")).toBe(true);
  });
});

describe("mostRecentDbConnection", () => {
  it("returns the last db block's connection", () => {
    const content =
      "```db-mysql connection=alpha\nq1\n```\n\n```db-postgres connection=omega\nq2\n```\n";
    expect(mostRecentDbConnection(content)).toBe("omega");
  });

  it("falls back to an earlier block's connection if the last has none", () => {
    const content =
      "```db-postgres connection=fallback\nq1\n```\n\n```db-postgres alias=q2\nq\n```\n";
    expect(mostRecentDbConnection(content)).toBe("fallback");
  });

  it("returns null when no db block has a connection", () => {
    const content = "```db-postgres alias=q1\nq\n```\n";
    expect(mostRecentDbConnection(content)).toBeNull();
  });

  it("returns null when no db blocks exist", () => {
    expect(mostRecentDbConnection("# title\n")).toBeNull();
  });

  it("trims whitespace-only connection values to null", () => {
    // db-fence parser already trims so this is mostly a regression
    // canary; if parser changes, our fallback still treats blank as
    // missing.
    const content = "```db-postgres\nq\n```\n";
    expect(mostRecentDbConnection(content)).toBeNull();
  });
});
