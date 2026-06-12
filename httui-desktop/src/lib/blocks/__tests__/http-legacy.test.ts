import { describe, expect, it } from "vitest";

import {
  isLegacyHttpBody,
  parseLegacyHttpBody,
  legacyToHttpMessage,
} from "../http-legacy";

describe("isLegacyHttpBody / parseLegacyHttpBody", () => {
  it("detects legacy JSON body with method+url", () => {
    const body =
      '{"method":"POST","url":"https://api.test.com/login","params":[],"headers":[],"body":""}';
    expect(isLegacyHttpBody(body)).toBe(true);
    const parsed = parseLegacyHttpBody(body);
    expect(parsed?.method).toBe("POST");
    expect(parsed?.url).toBe("https://api.test.com/login");
    expect(parsed?.params).toEqual([]);
    expect(parsed?.headers).toEqual([]);
  });

  it("treats raw HTTP message as non-legacy", () => {
    expect(isLegacyHttpBody("GET https://example.com")).toBe(false);
    expect(parseLegacyHttpBody("GET https://example.com")).toBeNull();
  });

  it("treats empty body as non-legacy", () => {
    expect(isLegacyHttpBody("")).toBe(false);
  });

  it("treats malformed JSON as non-legacy", () => {
    expect(isLegacyHttpBody("{not valid")).toBe(false);
  });

  it("treats JSON without method/url as non-legacy", () => {
    expect(isLegacyHttpBody('{"foo":"bar"}')).toBe(false);
  });

  it("rejects bodies that don't start with `{` after trim", () => {
    expect(isLegacyHttpBody('// comment\n{"method":"GET","url":"x"}')).toBe(
      false,
    );
  });

  it("rejects unknown methods", () => {
    expect(isLegacyHttpBody('{"method":"FETCH","url":"x"}')).toBe(false);
  });

  it("normalizes method to uppercase", () => {
    expect(parseLegacyHttpBody('{"method":"post","url":"x"}')?.method).toBe(
      "POST",
    );
  });

  it("extracts params and headers arrays", () => {
    const body = JSON.stringify({
      method: "POST",
      url: "https://example.com",
      params: [{ key: "page", value: "1" }],
      headers: [{ key: "Authorization", value: "Bearer x" }],
      body: '{"a":1}',
    });
    const parsed = parseLegacyHttpBody(body)!;
    expect(parsed.params).toEqual([{ key: "page", value: "1" }]);
    expect(parsed.headers).toEqual([
      { key: "Authorization", value: "Bearer x" },
    ]);
    expect(parsed.body).toBe('{"a":1}');
  });

  it("accepts both snake_case and camelCase timeout", () => {
    expect(
      parseLegacyHttpBody('{"method":"GET","url":"x","timeout_ms":5000}')
        ?.timeoutMs,
    ).toBe(5000);
    expect(
      parseLegacyHttpBody('{"method":"GET","url":"x","timeoutMs":5000}')
        ?.timeoutMs,
    ).toBe(5000);
  });

  it("legacyToHttpMessage converts to new shape with all rows enabled", () => {
    const legacy = parseLegacyHttpBody(
      JSON.stringify({
        method: "POST",
        url: "https://example.com",
        params: [{ key: "page", value: "1" }],
        headers: [{ key: "Accept", value: "json" }],
        body: "hi",
      }),
    )!;
    const msg = legacyToHttpMessage(legacy);
    expect(msg).toEqual({
      method: "POST",
      url: "https://example.com",
      params: [{ key: "page", value: "1", enabled: true }],
      headers: [{ key: "Accept", value: "json", enabled: true }],
      body: "hi",
    });
  });
});
