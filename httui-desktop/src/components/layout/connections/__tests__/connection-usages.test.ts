import { describe, it, expect } from "vitest";

import {
  findUsagesAcrossVault,
  findUsagesInFile,
} from "@/components/layout/connections/connection-usages";

describe("findUsagesInFile", () => {
  it("returns no matches for a file without db blocks", () => {
    const md = "# title\n\nsome text\n";
    expect(findUsagesInFile("note.md", md, "c1")).toEqual([]);
  });

  it("matches a fenced block opened with `db-<id>`", () => {
    const md = [
      "intro",
      "```db-c1",
      "SELECT * FROM users;",
      "```",
      "outro",
    ].join("\n");
    const usages = findUsagesInFile("note.md", md, "c1");
    expect(usages).toHaveLength(1);
    expect(usages[0]).toMatchObject({
      filePath: "note.md",
      line: 2,
      preview: "SELECT * FROM users;",
    });
  });

  it("does not match a different connection id", () => {
    const md = "```db-c1\nSELECT 1;\n```\n";
    expect(findUsagesInFile("note.md", md, "c2")).toEqual([]);
  });

  it("does not match a connection id that is a prefix of the fence id", () => {
    // `db-c10` should NOT count as a match for connection `c1`.
    const md = "```db-c10\nSELECT 1;\n```\n";
    expect(findUsagesInFile("note.md", md, "c1")).toEqual([]);
  });

  it("matches when the info-string carries extra tokens", () => {
    const md = "```db-c1 alias=x timeout=5000\nSELECT 1;\n```\n";
    expect(findUsagesInFile("note.md", md, "c1")).toHaveLength(1);
  });

  it("does not match HTTP fences", () => {
    const md = "```http\nGET https://x\n```\n";
    expect(findUsagesInFile("note.md", md, "c1")).toEqual([]);
  });

  it("ignores fence-shaped lines that are inside another open fence", () => {
    // Inside an open fence, ```db-c1 is content not an opening.
    const md = [
      "```http",
      "GET https://x",
      "# this looks like ```db-c1 but it's body content",
      "```",
    ].join("\n");
    // Only the http closing fence + no db-c1 match
    expect(findUsagesInFile("note.md", md, "c1")).toEqual([]);
  });

  it("captures multiple usages in a single file", () => {
    const md = [
      "```db-c1",
      "SELECT 1;",
      "```",
      "",
      "```db-c1 alias=second",
      "SELECT 2;",
      "```",
    ].join("\n");
    const usages = findUsagesInFile("note.md", md, "c1");
    expect(usages).toHaveLength(2);
    expect(usages[0].line).toBe(1);
    expect(usages[1].line).toBe(5);
  });

  it("yields preview = null when the next line is empty", () => {
    const md = "```db-c1\n\n```\n";
    const usages = findUsagesInFile("note.md", md, "c1");
    expect(usages[0].preview).toBeNull();
  });

  it("truncates a long preview to 80 chars", () => {
    const long = "SELECT " + "x".repeat(200);
    const md = `\`\`\`db-c1\n${long}\n\`\`\`\n`;
    const usages = findUsagesInFile("note.md", md, "c1");
    expect(usages[0].preview).toHaveLength(80);
  });

  it("uses 1-based line numbers (first line is 1)", () => {
    const md = "```db-c1\nSELECT 1;\n```\n";
    expect(findUsagesInFile("note.md", md, "c1")[0].line).toBe(1);
  });
});

describe("findUsagesAcrossVault", () => {
  it("walks each (path, content) and aggregates", () => {
    const files = [
      { path: "a.md", content: "```db-c1\nSELECT 1;\n```\n" },
      { path: "b.md", content: "```db-c2\nSELECT 2;\n```\n" },
      { path: "c.md", content: "```db-c1\nSELECT 3;\n```\n" },
    ];
    const usages = findUsagesAcrossVault(files, "c1");
    expect(usages).toHaveLength(2);
    expect(usages.map((u) => u.filePath)).toEqual(["a.md", "c.md"]);
  });

  it("returns empty when no file matches", () => {
    const files = [{ path: "a.md", content: "no fences here\n" }];
    expect(findUsagesAcrossVault(files, "c1")).toEqual([]);
  });
});
