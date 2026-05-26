import { describe, it, expect } from "vitest";
import {
  toCurl,
  toFetch,
  toPython,
  toHTTPie,
  toHttpFile,
} from "../http-codegen";
import type { HttpMessageParsed } from "../http-fence";

const GET_SIMPLE: HttpMessageParsed = {
  method: "GET",
  url: "https://api.example.com/users",
  params: [{ key: "page", value: "1", enabled: true }],
  headers: [{ key: "Authorization", value: "Bearer xyz", enabled: true }],
  body: "",
};

const POST_JSON: HttpMessageParsed = {
  method: "POST",
  url: "https://api.example.com/users",
  params: [],
  headers: [{ key: "Content-Type", value: "application/json", enabled: true }],
  body: '{"name":"alice"}',
};

const POST_TRICKY: HttpMessageParsed = {
  method: "POST",
  url: "https://api.example.com/it's-fine",
  params: [
    { key: "q", value: "hello world", enabled: true },
    { key: "skip", value: "x", enabled: false },
  ],
  headers: [
    { key: "X-Auth", value: "tok 'with' quote", enabled: true },
    { key: "X-Off", value: "v", enabled: false },
  ],
  body: `{"line1":"a","line2":"b"}`,
};

describe("toCurl", () => {
  it("emits a single -X flag and the URL with query string", () => {
    const out = toCurl(GET_SIMPLE);
    expect(out).toContain("curl -X GET 'https://api.example.com/users?page=1'");
    expect(out).toContain("-H 'Authorization: Bearer xyz'");
    expect(out).not.toContain("--data-raw");
  });

  it("includes --data-raw for POST/PUT/PATCH/DELETE with body", () => {
    const out = toCurl(POST_JSON);
    expect(out).toContain("curl -X POST 'https://api.example.com/users'");
    expect(out).toContain("-H 'Content-Type: application/json'");
    expect(out).toContain(`--data-raw '{"name":"alice"}'`);
  });

  it("escapes single quotes inside values via close-escape-reopen", () => {
    const out = toCurl(POST_TRICKY);
    // URL contains an apostrophe — single-quote escape uses '\''
    expect(out).toContain("'https://api.example.com/it'\\''s-fine");
    // Header value with single quote escaped
    expect(out).toContain("'X-Auth: tok '\\''with'\\'' quote'");
  });

  it("URL-encodes spaces in query values", () => {
    const out = toCurl(POST_TRICKY);
    expect(out).toContain("q=hello%20world");
  });

  it("drops disabled rows", () => {
    const out = toCurl(POST_TRICKY);
    expect(out).not.toContain("skip=");
    expect(out).not.toContain("X-Off");
  });

  it("does NOT emit --data-raw for GET even when body has text", () => {
    // Some users leave a body around when toggling methods; the generator
    // must not silently include it for GET.
    const get = { ...GET_SIMPLE, body: "leftover" };
    expect(toCurl(get)).not.toContain("--data-raw");
  });
});

describe("toFetch", () => {
  it("emits valid JS", () => {
    const out = toFetch(POST_JSON);
    expect(out).toContain("await fetch('https://api.example.com/users', {");
    expect(out).toContain("method: 'POST'");
    expect(out).toContain("headers: {");
    expect(out).toContain("'Content-Type': 'application/json'");
    // Body is JSON with double quotes — single-quoted JS literal needs no
    // escape since `"` is fine inside `'...'`.
    expect(out).toContain(`body: '{"name":"alice"}'`);
  });

  it("escapes backslashes and single quotes", () => {
    const out = toFetch({
      ...POST_JSON,
      body: `back\\slash and 'quote'`,
    });
    expect(out).toContain(`body: 'back\\\\slash and \\'quote\\''`);
  });

  it("omits headers/body when empty", () => {
    const out = toFetch({
      method: "GET",
      url: "https://example.com",
      params: [],
      headers: [],
      body: "",
    });
    expect(out).not.toContain("headers:");
    expect(out).not.toContain("body:");
  });
});

describe("toPython", () => {
  it("imports requests and uses the right method function", () => {
    const out = toPython(POST_JSON);
    expect(out.startsWith("import requests")).toBe(true);
    expect(out).toContain("response = requests.post(");
    expect(out).toContain("'https://api.example.com/users',");
    expect(out).toContain("'Content-Type': 'application/json'");
  });

  it("uses params= for the query string and data= for the body", () => {
    const out = toPython(GET_SIMPLE);
    expect(out).toContain("params={");
    expect(out).toContain("'page': '1'");
    expect(out).not.toContain("data=");
  });

  it("escapes apostrophes and backslashes", () => {
    const out = toPython(POST_TRICKY);
    expect(out).toContain("'X-Auth': 'tok \\'with\\' quote'");
  });
});

describe("toHTTPie", () => {
  it("uses == for query params and : for headers", () => {
    const out = toHTTPie(GET_SIMPLE);
    expect(out).toContain("http GET 'https://api.example.com/users'");
    expect(out).toContain("'page==1'");
    expect(out).toContain("'Authorization:Bearer xyz'");
  });

  it("uses --raw for non-trivial body", () => {
    const out = toHTTPie(POST_JSON);
    expect(out).toContain('--raw=\'{"name":"alice"}\'');
  });
});

describe("toHttpFile", () => {
  it("emits the canonical HTTP message body", () => {
    const out = toHttpFile(POST_JSON);
    expect(out).toContain("POST https://api.example.com/users");
    expect(out).toContain("Content-Type: application/json");
    expect(out).toContain('{"name":"alice"}');
    expect(out.endsWith("\n")).toBe(true);
  });
});
