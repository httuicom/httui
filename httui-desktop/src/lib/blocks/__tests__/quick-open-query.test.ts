import { describe, expect, it } from "vitest";

import {
  applyTagQuery,
  parseQuickOpenQuery,
} from "@/lib/blocks/quick-open-query";

describe("parseQuickOpenQuery", () => {
  it("returns empty for empty / whitespace input", () => {
    expect(parseQuickOpenQuery("")).toEqual({ kind: "empty" });
    expect(parseQuickOpenQuery("   ")).toEqual({ kind: "empty" });
    expect(parseQuickOpenQuery("\t\n")).toEqual({ kind: "empty" });
  });

  it("parses a single #tag as tag mode", () => {
    expect(parseQuickOpenQuery("#payments")).toEqual({
      kind: "tag",
      tag: "payments",
    });
  });

  it("supports tag names with digits, dashes, underscores", () => {
    expect(parseQuickOpenQuery("#auth_v2")).toEqual({
      kind: "tag",
      tag: "auth_v2",
    });
    expect(parseQuickOpenQuery("#stripe-live")).toEqual({
      kind: "tag",
      tag: "stripe-live",
    });
    expect(parseQuickOpenQuery("#v2")).toEqual({
      kind: "tag",
      tag: "v2",
    });
  });

  it("rejects invalid tag shapes (treats as fuzzy)", () => {
    // Tag must start with letter/digit/underscore, no leading dash.
    expect(parseQuickOpenQuery("#-foo")).toEqual({
      kind: "fuzzy",
      value: "#-foo",
    });
    // Just `#` — fuzzy.
    expect(parseQuickOpenQuery("#")).toEqual({
      kind: "fuzzy",
      value: "#",
    });
  });

  it("falls back to fuzzy for plain text queries", () => {
    expect(parseQuickOpenQuery("payments")).toEqual({
      kind: "fuzzy",
      value: "payments",
    });
    expect(parseQuickOpenQuery("apt purr")).toEqual({
      kind: "fuzzy",
      value: "apt purr",
    });
  });

  it("trims surrounding whitespace before classification", () => {
    expect(parseQuickOpenQuery("  #payments  ")).toEqual({
      kind: "tag",
      tag: "payments",
    });
  });

  it("recognizes #tag OR #tag boolean", () => {
    expect(parseQuickOpenQuery("#payments OR #debug")).toEqual({
      kind: "tag-bool",
      op: "or",
      tags: ["payments", "debug"],
    });
  });

  it("recognizes #tag AND #tag boolean", () => {
    expect(parseQuickOpenQuery("#payments AND #debug")).toEqual({
      kind: "tag-bool",
      op: "and",
      tags: ["payments", "debug"],
    });
  });

  it("OR/AND keywords are case-insensitive", () => {
    expect(parseQuickOpenQuery("#a or #b")).toEqual({
      kind: "tag-bool",
      op: "or",
      tags: ["a", "b"],
    });
    expect(parseQuickOpenQuery("#a And #b")).toEqual({
      kind: "tag-bool",
      op: "and",
      tags: ["a", "b"],
    });
  });

  it("supports 3+ tags joined with the same operator", () => {
    expect(parseQuickOpenQuery("#a OR #b OR #c")).toEqual({
      kind: "tag-bool",
      op: "or",
      tags: ["a", "b", "c"],
    });
  });

  it("mixed operators fall back to fuzzy (ambiguous)", () => {
    expect(parseQuickOpenQuery("#a OR #b AND #c")).toEqual({
      kind: "fuzzy",
      value: "#a OR #b AND #c",
    });
  });

  it("non-tag operand in a bool slot falls back to fuzzy", () => {
    expect(parseQuickOpenQuery("foo OR #b")).toEqual({
      kind: "fuzzy",
      value: "foo OR #b",
    });
  });
});

describe("applyTagQuery", () => {
  const index: Record<string, string[]> = {
    payments: ["a.md", "b.md", "c.md"],
    debug: ["b.md", "d.md"],
    auth: [],
  };
  const byTag = (tag: string) => index[tag] ?? [];

  it("returns the byTag set for single-tag queries", () => {
    const r = applyTagQuery({ kind: "tag", tag: "payments" }, byTag);
    expect(r).toEqual(["a.md", "b.md", "c.md"]);
  });

  it("returns empty array for unknown tag", () => {
    const r = applyTagQuery({ kind: "tag", tag: "unknown" }, byTag);
    expect(r).toEqual([]);
  });

  it("applyTagQuery returns [] for non-tag queries", () => {
    expect(applyTagQuery({ kind: "fuzzy", value: "x" }, byTag)).toEqual([]);
    expect(applyTagQuery({ kind: "empty" }, byTag)).toEqual([]);
  });

  it("OR-mode unions sets in first-seen order, dedupes", () => {
    const r = applyTagQuery(
      { kind: "tag-bool", op: "or", tags: ["payments", "debug"] },
      byTag,
    );
    // a (payments), b (payments — debug skipped as dup), c
    // (payments), d (debug)
    expect(r).toEqual(["a.md", "b.md", "c.md", "d.md"]);
  });

  it("AND-mode keeps intersection in first-tag order", () => {
    const r = applyTagQuery(
      { kind: "tag-bool", op: "and", tags: ["payments", "debug"] },
      byTag,
    );
    // Only b.md is in both.
    expect(r).toEqual(["b.md"]);
  });

  it("AND-mode with empty intersection returns []", () => {
    const r = applyTagQuery(
      { kind: "tag-bool", op: "and", tags: ["payments", "auth"] },
      byTag,
    );
    expect(r).toEqual([]);
  });

  it("AND-mode with single tag mirrors single-tag query", () => {
    const r = applyTagQuery(
      { kind: "tag-bool", op: "and", tags: ["payments"] },
      byTag,
    );
    expect(r).toEqual(["a.md", "b.md", "c.md"]);
  });

  it("empty tags list returns []", () => {
    const r = applyTagQuery({ kind: "tag-bool", op: "or", tags: [] }, byTag);
    expect(r).toEqual([]);
  });
});
