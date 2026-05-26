import { describe, it, expect } from "vitest";

import {
  parseUrlEncoded,
  stringifyUrlEncoded,
} from "@/components/blocks/http/fenced/HttpFormTables";

describe("parseUrlEncoded", () => {
  it("returns [] for empty / whitespace bodies", () => {
    expect(parseUrlEncoded("")).toEqual([]);
    expect(parseUrlEncoded("   ")).toEqual([]);
  });

  it("splits key=value pairs and keeps value-less keys", () => {
    expect(parseUrlEncoded("a=1&b=2")).toEqual([
      { key: "a", value: "1" },
      { key: "b", value: "2" },
    ]);
    expect(parseUrlEncoded("flag&x=9")).toEqual([
      { key: "flag", value: "" },
      { key: "x", value: "9" },
    ]);
  });

  it("preserves '=' inside the value (only the first splits)", () => {
    expect(parseUrlEncoded("token=a=b=c")).toEqual([
      { key: "token", value: "a=b=c" },
    ]);
  });

  it("drops segments with an empty key", () => {
    expect(parseUrlEncoded("=orphan&ok=1")).toEqual([
      { key: "ok", value: "1" },
    ]);
  });
});

describe("stringifyUrlEncoded", () => {
  it("joins rows, dropping empty keys and bare value-less keys", () => {
    expect(
      stringifyUrlEncoded([
        { key: "a", value: "1" },
        { key: "", value: "x" },
        { key: "flag", value: "" },
      ]),
    ).toBe("a=1&flag");
  });

  it("round-trips a normal body", () => {
    const body = "a=1&b=2&flag";
    expect(stringifyUrlEncoded(parseUrlEncoded(body))).toBe(body);
  });
});
