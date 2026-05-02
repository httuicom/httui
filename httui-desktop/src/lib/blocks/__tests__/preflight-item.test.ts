import { describe, expect, it } from "vitest";

import {
  parsePreflightItem,
  stringifyPreflightItem,
} from "@/lib/blocks/preflight-item";

describe("parsePreflightItem", () => {
  it("parses an unchecked item", () => {
    expect(parsePreflightItem("[ ] do thing")).toEqual({
      text: "do thing",
      done: false,
    });
  });

  it("parses a checked item", () => {
    expect(parsePreflightItem("[x] done thing")).toEqual({
      text: "done thing",
      done: true,
    });
  });

  it("accepts an uppercase X", () => {
    expect(parsePreflightItem("[X] also done")).toEqual({
      text: "also done",
      done: true,
    });
  });

  it("treats a plain string as unchecked", () => {
    expect(parsePreflightItem("just text")).toEqual({
      text: "just text",
      done: false,
    });
  });

  it("trims surrounding whitespace from the label", () => {
    expect(parsePreflightItem("[x]   trailing   ")).toEqual({
      text: "trailing",
      done: true,
    });
  });
});

describe("stringifyPreflightItem", () => {
  it("emits the `[ ]` prefix for unchecked items", () => {
    expect(stringifyPreflightItem({ text: "todo", done: false })).toBe(
      "[ ] todo",
    );
  });

  it("emits the `[x]` prefix for checked items", () => {
    expect(stringifyPreflightItem({ text: "done", done: true })).toBe(
      "[x] done",
    );
  });

  it("round-trips through parse + stringify", () => {
    const original = { text: "Check x", done: true };
    expect(parsePreflightItem(stringifyPreflightItem(original))).toEqual(
      original,
    );
  });
});
