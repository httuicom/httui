import { describe, it, expect } from "vitest";
import {
  parseHttpFenceInfo,
  stringifyHttpFenceInfo,
  parseHttpMessageBody,
  stringifyHttpMessageBody,
  isLegacyHttpBody,
  parseLegacyHttpBody,
  legacyToHttpMessage,
  deriveBodyMode,
  setContentTypeForMode,
  isCompatibleSwitch,
  type HttpBlockMetadata,
  type HttpMessageParsed,
} from "../http-fence";

describe("parseHttpFenceInfo", () => {
  it("returns empty meta for `http` alone", () => {
    expect(parseHttpFenceInfo("http")).toEqual({});
  });

  it("returns null for non-http heads", () => {
    expect(parseHttpFenceInfo("db-postgres alias=x")).toBeNull();
    expect(parseHttpFenceInfo("e2e")).toBeNull();
    expect(parseHttpFenceInfo("javascript")).toBeNull();
    expect(parseHttpFenceInfo("")).toBeNull();
  });

  it("parses a complete canonical info string", () => {
    const meta = parseHttpFenceInfo(
      "http alias=req1 timeout=30000 display=split mode=raw",
    );
    expect(meta).toEqual({
      alias: "req1",
      timeoutMs: 30000,
      displayMode: "split",
      mode: "raw",
    });
  });

  it("is order-independent on read", () => {
    const a = parseHttpFenceInfo("http alias=req1 timeout=5000 display=output");
    const b = parseHttpFenceInfo("http display=output alias=req1 timeout=5000");
    expect(a).toEqual(b);
  });

  it("accepts legacy displayMode key", () => {
    const meta = parseHttpFenceInfo("http alias=x displayMode=output");
    expect(meta?.displayMode).toBe("output");
  });

  it("ignores unknown keys silently", () => {
    const meta = parseHttpFenceInfo("http alias=req1 foo=bar timeout=1000");
    expect(meta).toEqual({ alias: "req1", timeoutMs: 1000 });
  });

  it("ignores invalid values silently", () => {
    const meta = parseHttpFenceInfo(
      "http alias=req1 timeout=-5 display=weird mode=odd",
    );
    expect(meta).toEqual({ alias: "req1" });
  });

  it("rejects zero-length values", () => {
    const meta = parseHttpFenceInfo("http alias= timeout=10");
    expect(meta).toEqual({ timeoutMs: 10 });
  });

  it("tolerates extra whitespace between tokens", () => {
    const meta = parseHttpFenceInfo("  http   alias=req1   timeout=5000  ");
    expect(meta).toEqual({ alias: "req1", timeoutMs: 5000 });
  });
});

describe("stringifyHttpFenceInfo", () => {
  it("emits `http` alone for empty meta", () => {
    expect(stringifyHttpFenceInfo({})).toBe("http");
  });

  it("emits canonical order regardless of property iteration order", () => {
    const meta: HttpBlockMetadata = {
      mode: "form",
      displayMode: "split",
      timeoutMs: 30000,
      alias: "req1",
    };
    expect(stringifyHttpFenceInfo(meta)).toBe(
      "http alias=req1 timeout=30000 display=split mode=form",
    );
  });

  it("omits undefined fields", () => {
    expect(
      stringifyHttpFenceInfo({ alias: "req1", displayMode: "input" }),
    ).toBe("http alias=req1 display=input");
  });
});

describe("info string roundtrip", () => {
  const cases: HttpBlockMetadata[] = [
    {},
    { alias: "req1" },
    {
      alias: "createUser",
      timeoutMs: 30000,
      displayMode: "split",
      mode: "form",
    },
    { timeoutMs: 5000 },
    { mode: "raw" },
  ];

  it.each(cases)("preserves shape for %o", (meta) => {
    const str = stringifyHttpFenceInfo(meta);
    const parsed = parseHttpFenceInfo(str);
    expect(parsed).toEqual(meta);
  });

  it("stringify is idempotent (two roundtrips = one)", () => {
    const meta: HttpBlockMetadata = {
      alias: "req1",
      timeoutMs: 5000,
      displayMode: "output",
      mode: "form",
    };
    const once = stringifyHttpFenceInfo(meta);
    const twice = stringifyHttpFenceInfo(parseHttpFenceInfo(once)!);
    expect(twice).toBe(once);
  });
});

// ─────────────────────── Body parsing ───────────────────────

describe("parseHttpMessageBody — basic shapes", () => {
  it("parses GET with no headers/body", () => {
    const parsed = parseHttpMessageBody("GET https://api.example.com/users");
    expect(parsed.method).toBe("GET");
    expect(parsed.url).toBe("https://api.example.com/users");
    expect(parsed.params).toEqual([]);
    expect(parsed.headers).toEqual([]);
    expect(parsed.body).toBe("");
  });

  it("parses inline query into params", () => {
    const parsed = parseHttpMessageBody(
      "GET https://api.example.com/users?page=1&limit=10",
    );
    expect(parsed.url).toBe("https://api.example.com/users");
    expect(parsed.params).toEqual([
      { key: "page", value: "1", enabled: true },
      { key: "limit", value: "10", enabled: true },
    ]);
  });

  it("parses query continuation lines", () => {
    const parsed = parseHttpMessageBody(
      "GET https://api.example.com/users\n?page=1\n&limit=10",
    );
    expect(parsed.url).toBe("https://api.example.com/users");
    expect(parsed.params).toEqual([
      { key: "page", value: "1", enabled: true },
      { key: "limit", value: "10", enabled: true },
    ]);
  });

  it("merges inline + continuation params", () => {
    const parsed = parseHttpMessageBody(
      "GET https://api.example.com/users?page=1\n&limit=10",
    );
    expect(parsed.params).toEqual([
      { key: "page", value: "1", enabled: true },
      { key: "limit", value: "10", enabled: true },
    ]);
  });

  it("parses headers in `Key: Value` form", () => {
    const parsed = parseHttpMessageBody(
      "GET https://api.example.com\nAuthorization: Bearer xyz\nAccept: application/json",
    );
    expect(parsed.headers).toEqual([
      { key: "Authorization", value: "Bearer xyz", enabled: true },
      { key: "Accept", value: "application/json", enabled: true },
    ]);
  });

  it("preserves colons in header values", () => {
    const parsed = parseHttpMessageBody(
      "GET https://example.com\nX-Custom: a:b:c",
    );
    expect(parsed.headers[0]).toEqual({
      key: "X-Custom",
      value: "a:b:c",
      enabled: true,
    });
  });

  it("parses body after the first blank line", () => {
    const parsed = parseHttpMessageBody(
      'POST https://api.example.com/users\nContent-Type: application/json\n\n{"name":"alice"}',
    );
    expect(parsed.method).toBe("POST");
    expect(parsed.headers).toEqual([
      { key: "Content-Type", value: "application/json", enabled: true },
    ]);
    expect(parsed.body).toBe('{"name":"alice"}');
  });

  it("preserves blank lines INSIDE body", () => {
    const parsed = parseHttpMessageBody(
      "POST https://example.com\nContent-Type: text/plain\n\nline1\n\nline3",
    );
    expect(parsed.body).toBe("line1\n\nline3");
  });

  it("strips trailing blank lines from body", () => {
    const parsed = parseHttpMessageBody(
      "POST https://example.com\n\nbody\n\n\n",
    );
    expect(parsed.body).toBe("body");
  });
});

describe("parseHttpMessageBody — comments", () => {
  it("`# desc:` (case-sensitive) attaches description to the next row", () => {
    const parsed = parseHttpMessageBody(
      [
        "GET https://example.com",
        "# desc: pagination index",
        "?page=1",
        "# desc: max rows",
        "&limit=10",
      ].join("\n"),
    );
    expect(parsed.params).toEqual([
      {
        key: "page",
        value: "1",
        enabled: true,
        description: "pagination index",
      },
      { key: "limit", value: "10", enabled: true, description: "max rows" },
    ]);
  });

  it("attaches description to the next header", () => {
    const parsed = parseHttpMessageBody(
      [
        "GET https://example.com",
        "# desc: bearer token",
        "Authorization: Bearer xyz",
      ].join("\n"),
    );
    expect(parsed.headers[0]).toEqual({
      key: "Authorization",
      value: "Bearer xyz",
      enabled: true,
      description: "bearer token",
    });
  });

  it("treats `# Key: Value` as a disabled header", () => {
    const parsed = parseHttpMessageBody(
      ["GET https://example.com", "# X-Debug: 1"].join("\n"),
    );
    expect(parsed.headers).toEqual([
      { key: "X-Debug", value: "1", enabled: false },
    ]);
  });

  it("treats `# &key=value` as a disabled query continuation", () => {
    const parsed = parseHttpMessageBody(
      ["GET https://example.com", "?page=1", "# &cursor=abc"].join("\n"),
    );
    expect(parsed.params).toEqual([
      { key: "page", value: "1", enabled: true },
      { key: "cursor", value: "abc", enabled: false },
    ]);
  });

  it("ignores free-form `#` comments without `:` or `?/&`", () => {
    const parsed = parseHttpMessageBody(
      [
        "GET https://example.com",
        "# this is just a note",
        "Authorization: Bearer x",
      ].join("\n"),
    );
    expect(parsed.headers).toEqual([
      { key: "Authorization", value: "Bearer x", enabled: true },
    ]);
  });

  it("`#desc:` (no space) is NOT treated as description (case-sensitive)", () => {
    const parsed = parseHttpMessageBody(
      ["GET https://example.com", "#desc: oops", "Accept: x"].join("\n"),
    );
    // The line is treated as a free-form `#desc: oops` (no space after `#`),
    // which doesn't match `# desc: ` exactly, so no description is attached.
    expect(parsed.headers).toEqual([
      { key: "Accept", value: "x", enabled: true },
    ]);
  });
});

describe("parseHttpMessageBody — edge cases", () => {
  it("empty body returns sane defaults", () => {
    const parsed = parseHttpMessageBody("");
    expect(parsed).toEqual({
      method: "GET",
      url: "",
      params: [],
      headers: [],
      body: "",
    });
  });

  it("only-whitespace body returns sane defaults", () => {
    const parsed = parseHttpMessageBody("\n\n   \n");
    expect(parsed.url).toBe("");
  });

  it("malformed first line falls back to defaults", () => {
    const parsed = parseHttpMessageBody("not a valid request line");
    expect(parsed).toEqual({
      method: "GET",
      url: "",
      params: [],
      headers: [],
      body: "",
    });
  });

  it("rejects unknown method keywords", () => {
    const parsed = parseHttpMessageBody("FETCH https://example.com");
    expect(parsed.url).toBe("");
  });

  it("supports all known methods", () => {
    for (const m of [
      "GET",
      "POST",
      "PUT",
      "PATCH",
      "DELETE",
      "HEAD",
      "OPTIONS",
    ]) {
      const parsed = parseHttpMessageBody(`${m} https://example.com`);
      expect(parsed.method).toBe(m);
    }
  });
});

// ─────────────────────── Body emission ───────────────────────

describe("stringifyHttpMessageBody — inline vs continuation", () => {
  it("emits inline query when short enough", () => {
    const parsed: HttpMessageParsed = {
      method: "GET",
      url: "https://api.example.com/users",
      params: [
        { key: "page", value: "1", enabled: true },
        { key: "limit", value: "10", enabled: true },
      ],
      headers: [],
      body: "",
    };
    expect(stringifyHttpMessageBody(parsed)).toBe(
      "GET https://api.example.com/users?page=1&limit=10",
    );
  });

  it("breaks to continuation when first line would exceed ~80 chars", () => {
    const longUrl = "https://api.example.com/some/long/resource/path/here";
    const parsed: HttpMessageParsed = {
      method: "GET",
      url: longUrl,
      params: [
        { key: "filter", value: "alpha", enabled: true },
        { key: "sort", value: "ascending", enabled: true },
      ],
      headers: [],
      body: "",
    };
    const out = stringifyHttpMessageBody(parsed);
    expect(out).toContain(`GET ${longUrl}`);
    expect(out).toMatch(/\n\?filter=alpha/);
    expect(out).toMatch(/\n&sort=ascending/);
  });

  it("forces continuation when any param is disabled", () => {
    const parsed: HttpMessageParsed = {
      method: "GET",
      url: "https://example.com",
      params: [
        { key: "page", value: "1", enabled: true },
        { key: "cursor", value: "abc", enabled: false },
      ],
      headers: [],
      body: "",
    };
    expect(stringifyHttpMessageBody(parsed)).toBe(
      "GET https://example.com\n?page=1\n# &cursor=abc",
    );
  });

  it("forces continuation when any param has description", () => {
    const parsed: HttpMessageParsed = {
      method: "GET",
      url: "https://example.com",
      params: [
        { key: "page", value: "1", enabled: true, description: "page index" },
      ],
      headers: [],
      body: "",
    };
    expect(stringifyHttpMessageBody(parsed)).toBe(
      "GET https://example.com\n# desc: page index\n?page=1",
    );
  });
});

describe("stringifyHttpMessageBody — headers, body, descriptions", () => {
  it("emits descriptions above headers", () => {
    const parsed: HttpMessageParsed = {
      method: "GET",
      url: "https://example.com",
      params: [],
      headers: [
        {
          key: "Authorization",
          value: "Bearer x",
          enabled: true,
          description: "bearer token",
        },
      ],
      body: "",
    };
    expect(stringifyHttpMessageBody(parsed)).toBe(
      "GET https://example.com\n# desc: bearer token\nAuthorization: Bearer x",
    );
  });

  it("disables headers with `# ` prefix", () => {
    const parsed: HttpMessageParsed = {
      method: "GET",
      url: "https://example.com",
      params: [],
      headers: [{ key: "X-Debug", value: "1", enabled: false }],
      body: "",
    };
    expect(stringifyHttpMessageBody(parsed)).toBe(
      "GET https://example.com\n# X-Debug: 1",
    );
  });

  it("emits one blank line before body", () => {
    const parsed: HttpMessageParsed = {
      method: "POST",
      url: "https://example.com/users",
      params: [],
      headers: [
        { key: "Content-Type", value: "application/json", enabled: true },
      ],
      body: '{"name":"alice"}',
    };
    expect(stringifyHttpMessageBody(parsed)).toBe(
      'POST https://example.com/users\nContent-Type: application/json\n\n{"name":"alice"}',
    );
  });
});

// ─────────────────────── Roundtrip / idempotency ───────────────────────

describe("body parse/stringify roundtrip", () => {
  const cases: Array<{ name: string; input: string }> = [
    {
      name: "GET no params",
      input: "GET https://example.com/users",
    },
    {
      name: "GET with inline query",
      input: "GET https://example.com/users?page=1&limit=10",
    },
    {
      name: "POST with body",
      input:
        'POST https://example.com/users\nContent-Type: application/json\n\n{"name":"alice"}',
    },
    {
      name: "params with disabled and descriptions",
      input: [
        "GET https://example.com/users",
        "# desc: pagination index",
        "?page=1",
        "# desc: max rows",
        "&limit=10",
        "# &cursor=abc",
        "Accept: application/json",
      ].join("\n"),
    },
  ];

  it.each(cases)("$name — fixed point after one reformat", ({ input }) => {
    const once = stringifyHttpMessageBody(parseHttpMessageBody(input));
    const twice = stringifyHttpMessageBody(parseHttpMessageBody(once));
    expect(twice).toBe(once);
  });

  it("inline query stays inline after roundtrip", () => {
    const input = "GET https://example.com?a=1&b=2";
    const out = stringifyHttpMessageBody(parseHttpMessageBody(input));
    expect(out).toBe(input);
  });
});

// ─────────────────────── Legacy JSON body ───────────────────────

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
