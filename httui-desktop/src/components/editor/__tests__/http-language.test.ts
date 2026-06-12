import { describe, expect, it } from "vitest";
import { LanguageDescription } from "@codemirror/language";
import { parser } from "@httui/lezer-http";

import {
  bodyContentType,
  httpLanguages,
  nestedModeFor,
} from "../http-language";

function findBody(input: string) {
  const tree = parser.parse(input);
  let body: ReturnType<typeof tree.cursor>["node"] | null = null;
  tree.iterate({
    enter(node) {
      if (node.name === "Body") body = node.node;
    },
  });
  return body;
}

const stringInput = (text: string) => ({
  length: text.length,
  chunk: (from: number) => text.slice(from),
  lineChunks: false,
  read: (from: number, to: number) => text.slice(from, to),
});

describe("httpLanguages", () => {
  it("registers the http fence language", () => {
    const match = LanguageDescription.matchLanguageName(httpLanguages, "http");
    expect(match?.name).toBe("http");
  });

  it("loads a LanguageSupport with the lezer parser", async () => {
    const support = await httpLanguages[0].load();
    expect(support.language.name).toBe("http");
    const tree = support.language.parser.parse("GET /x");
    expect(tree.toString()).toContain("GET");
  });

  it("parses a JSON body with the nested parser when Content-Type says so", async () => {
    const support = await httpLanguages[0].load();
    const doc = 'POST /x\nContent-Type: application/json\n\n{"a": [1, 2]}';
    const tree = support.language.parser.parse(doc);
    expect(tree.toString()).toContain("JsonText");
  });

  it("keeps the body opaque without a matching Content-Type", async () => {
    const support = await httpLanguages[0].load();
    const doc = "POST /x\nContent-Type: text/plain\n\n{not json}";
    const tree = support.language.parser.parse(doc);
    expect(tree.toString()).not.toContain("JsonText");
    expect(tree.toString()).toContain("BodyLine");
  });
});

describe("bodyContentType", () => {
  function contentTypeOf(doc: string): string | null {
    const body = findBody(doc);
    if (!body) return null;
    return bodyContentType(body, stringInput(doc) as never);
  }

  it("reads the enabled Content-Type header", () => {
    expect(contentTypeOf("POST /x\nContent-Type: application/json\n\n{}")).toBe(
      "application/json",
    );
  });

  it("matches the header name case-insensitively", () => {
    expect(contentTypeOf("POST /x\ncontent-type: text/xml\n\n<a/>")).toBe(
      "text/xml",
    );
  });

  it("ignores disabled Content-Type headers", () => {
    expect(
      contentTypeOf("POST /x\n# Content-Type: application/json\nA: b\n\n{}"),
    ).toBeNull();
  });

  it("returns null without headers", () => {
    expect(contentTypeOf("POST /x\n\n{}")).toBeNull();
  });
});

describe("nestedModeFor", () => {
  it("maps json content types", () => {
    expect(nestedModeFor("application/json")).toBe("json");
    expect(nestedModeFor("application/json; charset=utf-8")).toBe("json");
    expect(nestedModeFor("application/vnd.api+json")).toBe("json");
  });

  it("maps xml content types", () => {
    expect(nestedModeFor("application/xml")).toBe("xml");
    expect(nestedModeFor("text/xml")).toBe("xml");
    expect(nestedModeFor("image/svg+xml")).toBe("xml");
  });

  it("leaves other types opaque", () => {
    expect(nestedModeFor("text/plain")).toBeNull();
    expect(nestedModeFor("multipart/form-data")).toBeNull();
    expect(nestedModeFor("application/octet-stream")).toBeNull();
  });
});
