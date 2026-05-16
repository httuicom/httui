import { describe, it, expect } from "vitest";

import {
  findPrevBlock,
  resolveReference,
  resolveAllReferences,
  parseReferences,
  type BlockContext,
} from "@/lib/blocks/references";

function blk(
  alias: string,
  pos: number,
  response: unknown,
  blockType = "http",
): BlockContext {
  return {
    alias,
    blockType,
    pos,
    content: "",
    cachedResult:
      response === undefined
        ? null
        : { status: "200", response: JSON.stringify(response) },
  };
}

const ref = (raw: string) => parseReferences(raw)[0];

describe("findPrevBlock", () => {
  it("returns the block with the greatest pos below currentPos", () => {
    const blocks = [
      blk("a", 0, { x: 1 }),
      blk("b", 50, { x: 2 }),
      blk("c", 200, { x: 3 }),
    ];
    expect(findPrevBlock(blocks, 100)?.alias).toBe("b");
    expect(findPrevBlock(blocks, 10)?.alias).toBe("a");
    expect(findPrevBlock(blocks, 0)).toBeUndefined();
  });
});

describe("$prev resolution", () => {
  it("resolves against the previous block's response (response is the root)", () => {
    const blocks = [
      blk("first", 0, { body: { id: 7 } }),
      blk("second", 80, { body: { id: 99 } }),
    ];
    // currentPos after `second` → $prev = second
    expect(resolveReference(ref("{{$prev.body.id}}"), blocks, 200)).toBe("99");
    // currentPos between first and second → $prev = first
    expect(resolveReference(ref("{{$prev.body.id}}"), blocks, 50)).toBe("7");
  });

  it("errors clearly when there is no previous block", () => {
    expect(() =>
      resolveReference(ref("{{$prev.x}}"), [blk("a", 100, { x: 1 })], 10),
    ).toThrow(/no previous block/i);
  });

  it("errors when the previous block has not run yet", () => {
    const blocks = [blk("a", 0, undefined)];
    expect(() => resolveReference(ref("{{$prev.x}}"), blocks, 50)).toThrow(
      /run it first/i,
    );
  });

  it("errors on invalid cached JSON", () => {
    const blocks: BlockContext[] = [
      {
        alias: "a",
        blockType: "http",
        pos: 0,
        content: "",
        cachedResult: { status: "200", response: "{not json" },
      },
    ];
    expect(() => resolveReference(ref("{{$prev.x}}"), blocks, 50)).toThrow(
      /invalid cached response/i,
    );
  });

  it("resolveAllReferences substitutes $prev and never treats it as an env var", () => {
    const blocks = [blk("a", 0, { token: "abc" })];
    const out = resolveAllReferences(
      "Authorization: Bearer {{$prev.token}}",
      blocks,
      50,
      { $prev: "SHOULD_NOT_WIN" },
    );
    expect(out.errors).toHaveLength(0);
    expect(out.resolved).toBe("Authorization: Bearer abc");
  });

  it("surfaces the $prev error through resolveAllReferences", () => {
    const out = resolveAllReferences("{{$prev.x}}", [], 50);
    expect(out.errors).toHaveLength(1);
    expect(out.errors[0].message).toMatch(/no previous block/i);
  });

  it("works for db blocks via the stage-2 response view", () => {
    const dbResp = { results: [{ rows: [{ id: 42 }] }] };
    const blocks = [blk("q", 0, dbResp, "db-pg")];
    // $prev roots at response → the db view shim maps `.id` to row 0.
    expect(resolveReference(ref("{{$prev.id}}"), blocks, 50)).toBe("42");
  });
});
