import { describe, it, expect } from "vitest";
import {
  parseBody,
  deriveHost,
  httpElapsedOf,
  isValidHeaderName,
  buildExecutorParams,
} from "../http-request-builder";
import type { HttpMessageParsed } from "@/lib/blocks/http-message";
import type { HttpResponseFull } from "@/lib/tauri/streamedExecution";

const baseParsed = (
  overrides: Partial<HttpMessageParsed> = {},
): HttpMessageParsed => ({
  method: "GET",
  url: "https://api.example.com/users",
  params: [],
  headers: [],
  body: "",
  ...overrides,
});

describe("parseBody", () => {
  it("parses the post-redesign HTTP-message body shape", () => {
    const out = parseBody(
      'POST https://api.example.com/users\nContent-Type: application/json\n\n{"name":"a"}',
    );
    expect(out.method).toBe("POST");
    expect(out.url).toBe("https://api.example.com/users");
    expect(out.headers.some((h) => h.key === "Content-Type")).toBe(true);
    expect(out.body).toContain('"name"');
  });

  it("recognises and converts the legacy JSON body", () => {
    // legacyToHttpMessage round-trip: when the body is valid JSON
    // matching `{method,url,...}`, parseBody routes through the shim.
    const legacy = JSON.stringify({
      method: "GET",
      url: "https://api.example.com/legacy",
      headers: {},
    });
    const out = parseBody(legacy);
    expect(out.method).toBe("GET");
    expect(out.url).toBe("https://api.example.com/legacy");
  });

  it("returns a parsed shape even for an empty body", () => {
    const out = parseBody("");
    expect(out.method).toBeDefined();
    expect(out.url).toBeDefined();
  });
});

describe("deriveHost", () => {
  it("extracts the host for a fully-qualified URL", () => {
    expect(deriveHost("https://api.example.com/users?x=1")).toBe(
      "api.example.com",
    );
    expect(deriveHost("https://api.example.com:8080/x")).toBe(
      "api.example.com:8080",
    );
  });

  it("returns null for malformed URLs and empty input", () => {
    expect(deriveHost("")).toBeNull();
    expect(deriveHost("not a url")).toBeNull();
    expect(deriveHost("/relative/path")).toBeNull();
  });
});

describe("httpElapsedOf", () => {
  it("returns the response.elapsed_ms field", () => {
    const r = { elapsed_ms: 123 } as HttpResponseFull;
    expect(httpElapsedOf(r)).toBe(123);
  });

  it("returns undefined when the response has no elapsed_ms", () => {
    const r = {} as HttpResponseFull;
    expect(httpElapsedOf(r)).toBeUndefined();
  });
});

describe("isValidHeaderName", () => {
  it("accepts RFC 7230 token characters", () => {
    expect(isValidHeaderName("Authorization")).toBe(true);
    expect(isValidHeaderName("X-Custom-Header")).toBe(true);
    expect(isValidHeaderName("Content-Type")).toBe(true);
    expect(isValidHeaderName("foo!#$%&'*+-.^_`|~123")).toBe(true);
  });

  it("rejects whitespace and special characters", () => {
    expect(isValidHeaderName("with space")).toBe(false);
    expect(isValidHeaderName("with:colon")).toBe(false);
    expect(isValidHeaderName("with;semi")).toBe(false);
    expect(isValidHeaderName("")).toBe(false);
    expect(isValidHeaderName("{template}")).toBe(false);
  });
});

describe("buildExecutorParams", () => {
  const identity = (s: string) => s;

  it("forwards method, url, body and resolves text through resolveText", () => {
    const parsed = baseParsed({
      url: "https://api.{{HOST}}/v1",
      body: "{{TOKEN}}",
    });
    const upper = (s: string) =>
      s.replace("{{HOST}}", "example.com").replace("{{TOKEN}}", "abc");
    const { params, errors } = buildExecutorParams(parsed, upper, undefined);
    expect(errors).toEqual([]);
    expect(params.method).toBe("GET");
    expect(params.url).toBe("https://api.example.com/v1");
    expect(params.body).toBe("abc");
  });

  it("drops disabled and key-empty header / param rows", () => {
    const parsed = baseParsed({
      headers: [
        { key: "Authorization", value: "x", enabled: true },
        { key: "X-Off", value: "y", enabled: false },
        { key: "", value: "z", enabled: true },
      ],
      params: [
        { key: "page", value: "1", enabled: true },
        { key: "skip", value: "x", enabled: false },
        { key: "", value: "y", enabled: true },
      ],
    });
    const { params, errors } = buildExecutorParams(parsed, identity, undefined);
    expect(errors).toEqual([]);
    expect(params.headers).toEqual([{ key: "Authorization", value: "x" }]);
    expect(params.params).toEqual([{ key: "page", value: "1" }]);
  });

  it("emits validation errors for header names with spaces", () => {
    const parsed = baseParsed({
      headers: [{ key: "Bad Name", value: "x", enabled: true }],
    });
    const { params, errors } = buildExecutorParams(parsed, identity, undefined);
    expect(errors.length).toBe(1);
    expect(errors[0]).toContain('Invalid header name "Bad Name"');
    // The bad row is filtered out of params too.
    expect(params.headers).toEqual([]);
  });

  it("annotates the error when the bad name came from a {{ref}}", () => {
    const parsed = baseParsed({
      headers: [{ key: "{{H}}", value: "x", enabled: true }],
    });
    const { errors } = buildExecutorParams(
      parsed,
      (s) => s.replace("{{H}}", "Bad Name"),
      undefined,
    );
    expect(errors[0]).toContain('(resolved from "{{H}}")');
  });

  it("includes timeout_ms only when supplied", () => {
    const parsed = baseParsed();
    expect(
      buildExecutorParams(parsed, identity, undefined).params,
    ).not.toHaveProperty("timeout_ms");
    expect(buildExecutorParams(parsed, identity, 12345).params.timeout_ms).toBe(
      12345,
    );
  });

  it("forwards explicit-false transport overrides only (defaults stay backend-side)", () => {
    const parsed = baseParsed();
    // Empty settings → no overrides.
    expect(
      buildExecutorParams(parsed, identity, undefined, {}).params,
    ).not.toHaveProperty("follow_redirects");
    // Explicit false → forwarded.
    const { params } = buildExecutorParams(parsed, identity, undefined, {
      followRedirects: false,
      verifySsl: false,
      encodeUrl: false,
      trimWhitespace: false,
    });
    expect(params.follow_redirects).toBe(false);
    expect(params.verify_ssl).toBe(false);
    expect(params.encode_url).toBe(false);
    expect(params.trim_whitespace).toBe(false);
    // Explicit true → omitted (backend default).
    expect(
      buildExecutorParams(parsed, identity, undefined, {
        followRedirects: true,
      }).params,
    ).not.toHaveProperty("follow_redirects");
  });
});
