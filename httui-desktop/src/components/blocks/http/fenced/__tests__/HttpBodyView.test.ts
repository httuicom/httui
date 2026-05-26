import { describe, it, expect } from "vitest";

import {
  detectPreview,
  selectBodyLanguage,
  detectLang,
} from "@/components/blocks/http/fenced/HttpBodyView";
import type { HttpResponseFull } from "@/lib/tauri/streamedExecution";

// detectPreview only reads `headers` + `body`; the rest of
// HttpResponseFull is irrelevant to it, so a scoped cast keeps the
// fixture honest without spelling out timing/cookies.
const res = (
  headers: Record<string, string>,
  body: unknown,
): HttpResponseFull =>
  ({
    status_code: 200,
    status_text: "OK",
    headers,
    body,
    size_bytes: 0,
    elapsed_ms: 0,
  }) as unknown as HttpResponseFull;

describe("detectPreview", () => {
  it("detects base64 images and PDFs from the content-type", () => {
    expect(
      detectPreview(
        res(
          { "content-type": "image/png" },
          { encoding: "base64", data: "AA" },
        ),
      ),
    ).toEqual({
      kind: "image",
      dataUrl: "data:image/png;base64,AA",
      alt: "image/png",
    });

    expect(
      detectPreview(
        res(
          { "Content-Type": "application/pdf" },
          { encoding: "base64", data: "JV" },
        ),
      ),
    ).toEqual({ kind: "pdf", dataUrl: "data:application/pdf;base64,JV" });
  });

  it("treats text/html string bodies as html previews", () => {
    expect(
      detectPreview(
        res({ "content-type": "text/html; charset=utf-8" }, "<p>x</p>"),
      ),
    ).toEqual({ kind: "html", html: "<p>x</p>" });
  });

  it("returns none for JSON / unknown / base64-but-not-image", () => {
    expect(
      detectPreview(res({ "content-type": "application/json" }, "{}")).kind,
    ).toBe("none");
    expect(
      detectPreview(
        res(
          { "content-type": "application/octet-stream" },
          { encoding: "base64", data: "AA" },
        ),
      ).kind,
    ).toBe("none");
    expect(detectPreview(res({}, "plain")).kind).toBe("none");
  });
});

describe("detectLang", () => {
  it("returns json for valid JSON object/array starts", () => {
    expect(detectLang('{"a":1}', "pretty")).toBe("json");
    expect(detectLang("  [1,2]", "raw")).toBe("json");
  });

  it("returns xml for angle-bracket starts", () => {
    expect(detectLang("<root/>", "pretty")).toBe("xml");
    expect(detectLang("  <html>", "raw")).toBe("xml");
  });

  it("returns null for plain text and invalid JSON not starting with <", () => {
    expect(detectLang("hello world", "pretty")).toBeNull();
    // starts with '{' but invalid JSON and not '<' → falls through to null
    expect(detectLang("{nope", "raw")).toBeNull();
  });
});

describe("selectBodyLanguage", () => {
  it("maps content-type to a CM language extension (non-null)", () => {
    expect(selectBodyLanguage("application/json", "")).not.toBeNull();
    expect(selectBodyLanguage("text/xml", "")).not.toBeNull();
    expect(selectBodyLanguage("image/svg+xml", "")).not.toBeNull();
    expect(selectBodyLanguage("text/html", "")).not.toBeNull();
  });

  it("falls back to the body heuristic when content-type is absent/generic", () => {
    expect(selectBodyLanguage(null, '{"a":1}')).not.toBeNull(); // json heuristic
    expect(
      selectBodyLanguage("application/octet-stream", "<x/>"),
    ).not.toBeNull(); // xml heuristic
    expect(selectBodyLanguage(null, "plain text")).toBeNull();
  });
});
