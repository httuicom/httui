import { describe, expect, it } from "vitest";

import {
  findBlockForLine,
  findBlocksForHunkLines,
  findFencedBlocks,
} from "../find-blocks";

describe("findFencedBlocks", () => {
  it("returns empty for content with no fenced blocks", () => {
    expect(findFencedBlocks("just some text\n# heading\n")).toEqual([]);
  });

  it("finds an HTTP block with alias and timeout info-string tokens", () => {
    const md = [
      "intro",
      "```http alias=req1 timeout=30000",
      "GET https://api/users",
      "```",
      "outro",
    ].join("\n");
    const blocks = findFencedBlocks(md);
    expect(blocks).toHaveLength(1);
    expect(blocks[0]!.kind).toBe("http");
    expect(blocks[0]!.alias).toBe("req1");
    expect(blocks[0]!.startLine).toBe(2);
    expect(blocks[0]!.endLine).toBe(4);
  });

  it("finds a DB block with the connection id baked into the kind tag", () => {
    const md = ["```db-payments alias=q1", "SELECT 1", "```"].join("\n");
    const blocks = findFencedBlocks(md);
    expect(blocks).toHaveLength(1);
    expect(blocks[0]!.kind).toBe("db");
    expect(blocks[0]!.alias).toBe("q1");
    expect(blocks[0]!.infoString).toMatch(/db-payments/);
  });

  it("finds sh / ws / gql blocks", () => {
    const md = [
      "```sh",
      "echo hi",
      "```",
      "",
      "```ws alias=stream",
      "{}",
      "```",
      "",
      "```gql alias=q",
      "{ users { id } }",
      "```",
    ].join("\n");
    const kinds = findFencedBlocks(md).map((b) => b.kind);
    expect(kinds).toEqual(["sh", "ws", "gql"]);
  });

  it("returns null alias when info-string has no alias= token", () => {
    const md = ["```http", "GET /", "```"].join("\n");
    const blocks = findFencedBlocks(md);
    expect(blocks[0]!.alias).toBeNull();
  });

  it("finds multiple blocks in a single document", () => {
    const md = [
      "intro",
      "```http alias=a",
      "GET /a",
      "```",
      "between",
      "```http alias=b",
      "GET /b",
      "```",
    ].join("\n");
    const blocks = findFencedBlocks(md);
    expect(blocks.map((b) => b.alias)).toEqual(["a", "b"]);
  });

  it("ignores plain code fences (```ts, ```rust, plain ```)", () => {
    const md = ["```ts", "const x = 1", "```", "", "```", "plain", "```"].join(
      "\n",
    );
    expect(findFencedBlocks(md)).toEqual([]);
  });

  it("treats an unclosed fence as not-yet-a-block (no entry produced)", () => {
    const md = ["```http alias=req1", "GET /"].join("\n");
    expect(findFencedBlocks(md)).toEqual([]);
  });
});

describe("findBlockForLine", () => {
  const md = [
    "line 1",
    "```http alias=a", // line 2 — fence open
    "GET /", // line 3 — body
    "```", // line 4 — fence close
    "outside", // line 5
  ].join("\n");
  const blocks = findFencedBlocks(md);

  it("returns the block containing the line", () => {
    expect(findBlockForLine(blocks, 3)?.alias).toBe("a");
  });

  it("includes both fence lines as part of the block", () => {
    expect(findBlockForLine(blocks, 2)?.alias).toBe("a");
    expect(findBlockForLine(blocks, 4)?.alias).toBe("a");
  });

  it("returns null for lines outside any block", () => {
    expect(findBlockForLine(blocks, 1)).toBeNull();
    expect(findBlockForLine(blocks, 5)).toBeNull();
  });
});

describe("findBlocksForHunkLines", () => {
  const md = [
    "```http alias=a", // 1
    "GET /a", // 2
    "```", // 3
    "between", // 4
    "```http alias=b", // 5
    "GET /b", // 6
    "```", // 7
  ].join("\n");
  const blocks = findFencedBlocks(md);

  it("returns the unique blocks any of the lines fall into", () => {
    const matched = findBlocksForHunkLines(blocks, [2, 6]);
    expect(matched.map((b) => b.alias).sort()).toEqual(["a", "b"]);
  });

  it("dedupes when multiple lines hit the same block", () => {
    const matched = findBlocksForHunkLines(blocks, [1, 2, 3]);
    expect(matched).toHaveLength(1);
  });

  it("ignores lines outside any block", () => {
    expect(findBlocksForHunkLines(blocks, [4])).toEqual([]);
  });

  it("returns empty for empty input", () => {
    expect(findBlocksForHunkLines(blocks, [])).toEqual([]);
  });
});
