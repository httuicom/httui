import { describe, expect, it } from "vitest";

import {
  deriveBodyMode,
  setContentTypeForMode,
  isCompatibleSwitch,
} from "../http-body-modes";
import {
  stringifyHttpMessageBody,
  type HttpMessageParsed,
} from "../http-message";

describe("deriveBodyMode", () => {
  const empty: HttpMessageParsed = {
    method: "POST",
    url: "https://example.com",
    params: [],
    headers: [],
    body: "",
  };

  it("returns `none` when no Content-Type header is present", () => {
    expect(deriveBodyMode(empty.headers)).toBe("none");
  });

  it("returns `none` when Content-Type is disabled", () => {
    expect(
      deriveBodyMode([
        { key: "Content-Type", value: "application/json", enabled: false },
      ]),
    ).toBe("none");
  });

  it("recognises canonical mime types", () => {
    expect(
      deriveBodyMode([
        { key: "Content-Type", value: "application/json", enabled: true },
      ]),
    ).toBe("json");
    expect(
      deriveBodyMode([
        { key: "Content-Type", value: "application/xml", enabled: true },
      ]),
    ).toBe("xml");
    expect(
      deriveBodyMode([
        { key: "Content-Type", value: "text/xml", enabled: true },
      ]),
    ).toBe("xml");
    expect(
      deriveBodyMode([
        { key: "Content-Type", value: "text/plain", enabled: true },
      ]),
    ).toBe("text");
    expect(
      deriveBodyMode([
        {
          key: "Content-Type",
          value: "application/x-www-form-urlencoded",
          enabled: true,
        },
      ]),
    ).toBe("form-urlencoded");
    expect(
      deriveBodyMode([
        { key: "Content-Type", value: "multipart/form-data", enabled: true },
      ]),
    ).toBe("multipart");
    expect(
      deriveBodyMode([
        {
          key: "Content-Type",
          value: "application/octet-stream",
          enabled: true,
        },
      ]),
    ).toBe("binary");
  });

  it("recognises media-type suffixes (+json / +xml)", () => {
    expect(
      deriveBodyMode([
        {
          key: "Content-Type",
          value: "application/vnd.api+json",
          enabled: true,
        },
      ]),
    ).toBe("json");
    expect(
      deriveBodyMode([
        { key: "Content-Type", value: "application/atom+xml", enabled: true },
      ]),
    ).toBe("xml");
  });

  it("ignores parameters like charset and boundary", () => {
    expect(
      deriveBodyMode([
        {
          key: "Content-Type",
          value: "application/json; charset=utf-8",
          enabled: true,
        },
      ]),
    ).toBe("json");
    expect(
      deriveBodyMode([
        {
          key: "Content-Type",
          value: "multipart/form-data; boundary=----X",
          enabled: true,
        },
      ]),
    ).toBe("multipart");
  });

  it("uses case-insensitive header lookup", () => {
    expect(
      deriveBodyMode([
        { key: "content-type", value: "application/json", enabled: true },
      ]),
    ).toBe("json");
    expect(
      deriveBodyMode([
        { key: "CONTENT-TYPE", value: "application/json", enabled: true },
      ]),
    ).toBe("json");
  });

  it("classifies image/audio/video and pdf as binary", () => {
    expect(
      deriveBodyMode([
        { key: "Content-Type", value: "image/png", enabled: true },
      ]),
    ).toBe("binary");
    expect(
      deriveBodyMode([
        { key: "Content-Type", value: "audio/mpeg", enabled: true },
      ]),
    ).toBe("binary");
    expect(
      deriveBodyMode([
        { key: "Content-Type", value: "video/mp4", enabled: true },
      ]),
    ).toBe("binary");
    expect(
      deriveBodyMode([
        { key: "Content-Type", value: "application/pdf", enabled: true },
      ]),
    ).toBe("binary");
  });

  it("falls back to text for unknown text/* types", () => {
    expect(
      deriveBodyMode([
        { key: "Content-Type", value: "text/csv", enabled: true },
      ]),
    ).toBe("text");
  });
});

describe("setContentTypeForMode", () => {
  const base: HttpMessageParsed = {
    method: "POST",
    url: "https://example.com",
    params: [],
    headers: [],
    body: '{"a":1}',
  };

  it("inserts Content-Type when no header exists", () => {
    const out = setContentTypeForMode(base, "json");
    expect(out.headers).toEqual([
      { key: "Content-Type", value: "application/json", enabled: true },
    ]);
    expect(out.body).toBe(base.body);
    expect(out.url).toBe(base.url);
  });

  it("replaces an existing Content-Type value (case-insensitive lookup)", () => {
    const input: HttpMessageParsed = {
      ...base,
      headers: [
        { key: "Accept", value: "*/*", enabled: true },
        { key: "content-type", value: "text/plain", enabled: true },
      ],
    };
    const out = setContentTypeForMode(input, "json");
    expect(out.headers[0]).toEqual({
      key: "Accept",
      value: "*/*",
      enabled: true,
    });
    expect(out.headers[1].key).toBe("content-type");
    expect(out.headers[1].value).toBe("application/json");
    expect(out.headers[1].enabled).toBe(true);
  });

  it("preserves description when replacing", () => {
    const input: HttpMessageParsed = {
      ...base,
      headers: [
        {
          key: "Content-Type",
          value: "text/plain",
          enabled: true,
          description: "json please",
        },
      ],
    };
    const out = setContentTypeForMode(input, "json");
    expect(out.headers[0].description).toBe("json please");
    expect(out.headers[0].value).toBe("application/json");
  });

  it("re-enables a disabled Content-Type when switching to a real mode", () => {
    const input: HttpMessageParsed = {
      ...base,
      headers: [{ key: "Content-Type", value: "text/plain", enabled: false }],
    };
    const out = setContentTypeForMode(input, "json");
    expect(out.headers[0].enabled).toBe(true);
    expect(out.headers[0].value).toBe("application/json");
  });

  it("removes the header for `none`", () => {
    const input: HttpMessageParsed = {
      ...base,
      headers: [
        { key: "Accept", value: "*/*", enabled: true },
        { key: "Content-Type", value: "application/json", enabled: true },
      ],
    };
    const out = setContentTypeForMode(input, "none");
    expect(out.headers).toEqual([
      { key: "Accept", value: "*/*", enabled: true },
    ]);
  });

  it("is a no-op when removing from already-empty headers", () => {
    const out = setContentTypeForMode(base, "none");
    expect(out).toBe(base);
  });

  it("is a no-op when setting to the same mode", () => {
    const input: HttpMessageParsed = {
      ...base,
      headers: [
        { key: "Content-Type", value: "application/json", enabled: true },
      ],
    };
    const out = setContentTypeForMode(input, "json");
    expect(out).toBe(input);
  });

  it("is idempotent when called twice", () => {
    const a = setContentTypeForMode(base, "json");
    const b = setContentTypeForMode(a, "json");
    expect(stringifyHttpMessageBody(a)).toBe(stringifyHttpMessageBody(b));
  });

  it("never touches the body, url, params, or method", () => {
    const input: HttpMessageParsed = {
      method: "POST",
      url: "https://api.example.com/users",
      params: [{ key: "page", value: "1", enabled: true }],
      headers: [{ key: "Accept", value: "*/*", enabled: true }],
      body: "{ raw body that should stay }",
    };
    const out = setContentTypeForMode(input, "xml");
    expect(out.method).toBe("POST");
    expect(out.url).toBe(input.url);
    expect(out.params).toEqual(input.params);
    expect(out.body).toBe(input.body);
  });
});

describe("isCompatibleSwitch", () => {
  it("is compatible when prev === next", () => {
    expect(isCompatibleSwitch("json", "json", '{"a":1}')).toBe(true);
  });

  it("is compatible when body is empty", () => {
    expect(isCompatibleSwitch("json", "binary", "")).toBe(true);
    expect(isCompatibleSwitch("json", "binary", "   \n  ")).toBe(true);
  });

  it("is compatible when prev === none (just adding a declaration)", () => {
    expect(isCompatibleSwitch("none", "json", '{"a":1}')).toBe(true);
    expect(isCompatibleSwitch("none", "binary", "anything")).toBe(true);
  });

  it("is compatible between textual modes (json ↔ xml ↔ text)", () => {
    expect(isCompatibleSwitch("json", "xml", '{"a":1}')).toBe(true);
    expect(isCompatibleSwitch("xml", "text", "<a/>")).toBe(true);
    expect(isCompatibleSwitch("text", "json", "hello")).toBe(true);
  });

  it("is INCOMPATIBLE when going from textual to structured with non-empty body", () => {
    expect(isCompatibleSwitch("json", "form-urlencoded", '{"a":1}')).toBe(
      false,
    );
    expect(isCompatibleSwitch("text", "multipart", "hello")).toBe(false);
    expect(isCompatibleSwitch("xml", "binary", "<a/>")).toBe(false);
  });

  it("is compatible going structured → textual (no warning)", () => {
    expect(isCompatibleSwitch("multipart", "json", "irrelevant")).toBe(true);
    expect(isCompatibleSwitch("binary", "text", "irrelevant")).toBe(true);
  });
});
