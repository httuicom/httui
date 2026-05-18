import { describe, it, expect } from "vitest";
import {
  parseReferences,
  navigateJson,
  resolveAllReferences,
  resolveRefsToBindParams,
  type BlockContext,
} from "../references";

describe("parseReferences", () => {
  it("extracts a single reference", () => {
    const refs = parseReferences("GET /posts/{{login.response.body.id}}");
    expect(refs).toHaveLength(1);
    expect(refs[0].alias).toBe("login");
    expect(refs[0].path).toEqual(["response", "body", "id"]);
    expect(refs[0].raw).toBe("{{login.response.body.id}}");
  });

  it("extracts multiple references", () => {
    const refs = parseReferences("{{a.response.x}} and {{b.response.y}}");
    expect(refs).toHaveLength(2);
    expect(refs[0].alias).toBe("a");
    expect(refs[1].alias).toBe("b");
  });

  it("returns empty for text without references", () => {
    expect(parseReferences("just plain text")).toHaveLength(0);
  });

  it("handles alias-only reference (no path)", () => {
    const refs = parseReferences("{{myblock}}");
    expect(refs).toHaveLength(1);
    expect(refs[0].alias).toBe("myblock");
    expect(refs[0].path).toEqual([]);
  });

  it("trims whitespace in reference", () => {
    const refs = parseReferences("{{ login.response.body.id }}");
    expect(refs[0].alias).toBe("login");
    expect(refs[0].path).toEqual(["response", "body", "id"]);
  });
});

describe("navigateJson", () => {
  const data = {
    status_code: 200,
    body: {
      id: 42,
      items: [{ name: "first" }, { name: "second" }],
    },
  };

  it("navigates simple path", () => {
    expect(navigateJson(data, ["status_code"])).toBe(200);
  });

  it("navigates nested path", () => {
    expect(navigateJson(data, ["body", "id"])).toBe(42);
  });

  it("navigates into arrays by index", () => {
    expect(navigateJson(data, ["body", "items", "0", "name"])).toBe("first");
    expect(navigateJson(data, ["body", "items", "1", "name"])).toBe("second");
  });

  it("returns object if path ends at object", () => {
    expect(navigateJson(data, ["body"])).toEqual({
      id: 42,
      items: [{ name: "first" }, { name: "second" }],
    });
  });

  it("returns root if path is empty", () => {
    expect(navigateJson(data, [])).toBe(data);
  });

  it("throws on missing key", () => {
    expect(() => navigateJson(data, ["nonexistent"])).toThrow(
      'Key "nonexistent" not found',
    );
  });

  it("throws on out of bounds array index", () => {
    expect(() => navigateJson(data, ["body", "items", "5"])).toThrow(
      "out of bounds",
    );
  });

  it("throws on accessing property of primitive", () => {
    expect(() => navigateJson(data, ["status_code", "foo"])).toThrow(
      'Cannot access "foo" on number',
    );
  });
});

describe("resolveAllReferences", () => {
  const blocks: BlockContext[] = [
    {
      alias: "login",
      blockType: "http",
      pos: 10,
      content: "{}",
      cachedResult: {
        status: "success",
        response: JSON.stringify({
          status_code: 201,
          body: { id: 101, token: "abc123" },
        }),
      },
    },
    {
      alias: "users",
      blockType: "http",
      pos: 50,
      content: "{}",
      cachedResult: {
        status: "success",
        response: JSON.stringify({
          status_code: 200,
          body: [{ id: 1, name: "Alice" }],
        }),
      },
    },
  ];

  it("resolves a single reference", () => {
    const { resolved, errors } = resolveAllReferences(
      "/posts/{{login.response.body.id}}",
      blocks,
      100,
    );
    expect(errors).toHaveLength(0);
    expect(resolved).toBe("/posts/101");
  });

  it("resolves multiple references", () => {
    const { resolved, errors } = resolveAllReferences(
      "{{login.response.body.token}} for user {{users.response.body.0.name}}",
      blocks,
      100,
    );
    expect(errors).toHaveLength(0);
    expect(resolved).toBe("abc123 for user Alice");
  });

  it("returns text unchanged when no references", () => {
    const { resolved, errors } = resolveAllReferences(
      "plain text",
      blocks,
      100,
    );
    expect(resolved).toBe("plain text");
    expect(errors).toHaveLength(0);
  });

  it("returns error for unknown alias", () => {
    const { errors } = resolveAllReferences(
      "{{unknown.response.x}}",
      blocks,
      100,
    );
    expect(errors).toHaveLength(1);
    expect(errors[0].message).toContain("not found");
  });

  it("returns error for block below current position", () => {
    const { errors } = resolveAllReferences(
      "{{users.response.body}}",
      blocks,
      30, // users is at pos 50, current is at 30
    );
    expect(errors).toHaveLength(1);
    expect(errors[0].message).toContain("below");
  });

  it("returns error for block without cache", () => {
    const blocksNoCache: BlockContext[] = [
      {
        alias: "nocache",
        blockType: "http",
        pos: 5,
        content: "{}",
        cachedResult: null,
      },
    ];
    const { errors } = resolveAllReferences(
      "{{nocache.response.x}}",
      blocksNoCache,
      100,
    );
    expect(errors).toHaveLength(1);
    expect(errors[0].message).toContain("no result yet");
  });

  it("returns error for invalid JSON path", () => {
    const { errors } = resolveAllReferences(
      "{{login.response.body.nonexistent}}",
      blocks,
      100,
    );
    expect(errors).toHaveLength(1);
    expect(errors[0].message).toContain("not found");
  });

  it("serializes object values as JSON", () => {
    const { resolved } = resolveAllReferences(
      "{{login.response.body}}",
      blocks,
      100,
    );
    expect(resolved).toBe(JSON.stringify({ id: 101, token: "abc123" }));
  });

  it("resolves env variable when no matching block alias", () => {
    const envVars = { API_KEY: "secret-key" };
    const { resolved, errors } = resolveAllReferences(
      "Bearer {{API_KEY}}",
      blocks,
      100,
      envVars,
    );
    expect(errors).toHaveLength(0);
    expect(resolved).toBe("Bearer secret-key");
  });

  it("block reference takes priority over env var with same name", () => {
    const envVars = { login: "env-value-should-lose" };
    const { resolved, errors } = resolveAllReferences(
      "{{login.response.body.token}}",
      blocks,
      100,
      envVars,
    );
    expect(errors).toHaveLength(0);
    expect(resolved).toBe("abc123");
  });

  it("block alias without path takes priority over env var with same name", () => {
    // Block "login" exists and has cache, but {{login}} with no path navigates
    // the context root — should resolve as block ref, not env var
    const envVars = { login: "env-value-should-lose" };
    const { resolved, errors } = resolveAllReferences(
      "{{login}}",
      blocks,
      100,
      envVars,
    );
    expect(errors).toHaveLength(0);
    // resolveReference with empty path returns the full context as JSON
    expect(JSON.parse(resolved)).toHaveProperty("response");
  });

  it("falls back to env var when block has no cache and ref has no path", () => {
    const blocksNoCache: BlockContext[] = [
      {
        alias: "myvar",
        blockType: "http",
        pos: 5,
        content: "{}",
        cachedResult: null,
      },
    ];
    const envVars = { myvar: "from-env" };
    // Block exists but has no cache, and ref has no path → should NOT fall back to env
    // because block alias match takes priority (produces error about missing cache)
    const { errors } = resolveAllReferences(
      "{{myvar}}",
      blocksNoCache,
      100,
      envVars,
    );
    expect(errors).toHaveLength(1);
    expect(errors[0].message).toContain("no result yet");
  });

  // ─────── L166: alias/env shadow warning (T37) ───────────────

  it("emits warning when block alias shadows env var with same name", () => {
    // Block "login" + env var "login": block ref resolves correctly, but
    // a warning surfaces so the user knows their env var is being ignored.
    const envVars = { login: "from-env-shadowed" };
    const { resolved, errors, warnings } = resolveAllReferences(
      "{{login.response.body.token}}",
      blocks,
      100,
      envVars,
    );
    expect(errors).toHaveLength(0);
    expect(resolved).toBe("abc123");
    expect(warnings).toHaveLength(1);
    expect(warnings[0].message.toLowerCase()).toContain("shadow");
    expect(warnings[0].message).toContain("login");
  });

  it("no shadow warning when alias and env var names differ", () => {
    const envVars = { OTHER_KEY: "value" };
    const { warnings } = resolveAllReferences(
      "{{login.response.body.token}}",
      blocks,
      100,
      envVars,
    );
    expect(warnings).toHaveLength(0);
  });

  it("no shadow warning when only env var exists (no matching block)", () => {
    // {{API_KEY}} with no matching block alias — pure env resolution, no
    // shadowing happening, so no warning.
    const envVars = { API_KEY: "secret" };
    const { warnings } = resolveAllReferences(
      "Bearer {{API_KEY}}",
      blocks,
      100,
      envVars,
    );
    expect(warnings).toHaveLength(0);
  });
});

describe("db block reference shim (stage-2 response shape)", () => {
  const stage2Response = {
    results: [
      {
        kind: "select",
        columns: [
          { name: "id", type: "int" },
          { name: "name", type: "text" },
        ],
        rows: [
          { id: 7, name: "alice" },
          { id: 8, name: "bob" },
        ],
        has_more: false,
      },
      {
        kind: "mutation",
        rows_affected: 3,
      },
    ],
    messages: [],
    stats: { elapsed_ms: 12 },
  };

  const dbBlock: BlockContext = {
    alias: "q",
    blockType: "db-postgres",
    pos: 10,
    content: "",
    cachedResult: {
      status: "success",
      response: JSON.stringify(stage2Response),
    },
  };

  it("legacy shim: {{alias.response.col}} resolves to results[0].rows[0][col]", () => {
    const { resolved, errors } = resolveAllReferences(
      "user={{q.response.name}}",
      [dbBlock],
      100,
    );
    expect(errors).toHaveLength(0);
    expect(resolved).toBe("user=alice");
  });

  it("legacy shim works for numeric column values", () => {
    const { resolved, errors } = resolveAllReferences(
      "id={{q.response.id}}",
      [dbBlock],
      100,
    );
    expect(errors).toHaveLength(0);
    expect(resolved).toBe("id=7");
  });

  it("explicit multi-result: {{alias.response.0.rows.0.name}}", () => {
    const { resolved, errors } = resolveAllReferences(
      "{{q.response.0.rows.0.name}}",
      [dbBlock],
      100,
    );
    expect(errors).toHaveLength(0);
    expect(resolved).toBe("alice");
  });

  it("raw shape passthrough: {{alias.response.results.0.rows.0.id}}", () => {
    // Autocomplete walks the raw shape so users naturally type
    // `response.results.0.…`; the proxy must pass `results` through to
    // the underlying DbResponse for this path to resolve.
    const { resolved, errors } = resolveAllReferences(
      "{{q.response.results.0.rows.0.id}}",
      [dbBlock],
      100,
    );
    expect(errors).toHaveLength(0);
    expect(resolved).toBe("7");
  });

  it("raw shape passthrough: stats and messages", () => {
    const { resolved: elapsed, errors: e1 } = resolveAllReferences(
      "{{q.response.stats.elapsed_ms}}",
      [dbBlock],
      100,
    );
    expect(e1).toHaveLength(0);
    expect(elapsed).toBe("12");

    const { resolved: msgs, errors: e2 } = resolveAllReferences(
      "{{q.response.messages}}",
      [dbBlock],
      100,
    );
    expect(e2).toHaveLength(0);
    expect(msgs).toBe("[]");
  });

  it("explicit multi-result: {{alias.response.1.rows_affected}} reaches mutation result", () => {
    const { resolved, errors } = resolveAllReferences(
      "{{q.response.1.rows_affected}}",
      [dbBlock],
      100,
    );
    expect(errors).toHaveLength(0);
    expect(resolved).toBe("3");
  });

  it("explicit multi-result: second row via rows.1", () => {
    const { resolved, errors } = resolveAllReferences(
      "{{q.response.0.rows.1.name}}",
      [dbBlock],
      100,
    );
    expect(errors).toHaveLength(0);
    expect(resolved).toBe("bob");
  });

  it("out-of-range index raises a meaningful error", () => {
    const { errors } = resolveAllReferences(
      "{{q.response.5.rows}}",
      [dbBlock],
      100,
    );
    expect(errors).toHaveLength(1);
    expect(errors[0].message.toLowerCase()).toMatch(
      /(not found|out of bounds|undefined)/,
    );
  });

  it("legacy cache shape (pre-stage-2) still navigates directly", () => {
    const legacyBlock: BlockContext = {
      alias: "old",
      blockType: "db",
      pos: 5,
      content: "",
      cachedResult: {
        status: "success",
        response: JSON.stringify({
          columns: [{ name: "id", type: "int" }],
          rows: [{ id: 99 }],
          has_more: false,
        }),
      },
    };
    // Pre-stage-2 cache exposes columns/rows at the top level; the shim
    // only kicks in for the new shape, so legacy refs navigate raw.
    const { resolved, errors } = resolveAllReferences(
      "{{old.response.rows.0.id}}",
      [legacyBlock],
      100,
    );
    expect(errors).toHaveLength(0);
    expect(resolved).toBe("99");
  });

  it("shim does not apply to non-db blocks", () => {
    // An http block whose body happens to have a `results` field must NOT
    // be treated as a db response.
    const httpBlock: BlockContext = {
      alias: "http",
      blockType: "http",
      pos: 5,
      content: "",
      cachedResult: {
        status: "success",
        response: JSON.stringify({
          status_code: 200,
          results: [{ id: 1 }],
        }),
      },
    };
    const { resolved, errors } = resolveAllReferences(
      "{{http.response.results.0.id}}",
      [httpBlock],
      100,
    );
    expect(errors).toHaveLength(0);
    expect(resolved).toBe("1");
  });
});

describe("resolveRefsToBindParams", () => {
  const blocks: BlockContext[] = [
    {
      alias: "login",
      blockType: "http",
      pos: 5,
      content: "",
      cachedResult: {
        status: "success",
        response: JSON.stringify({ body: { id: 42, token: "abc" } }),
      },
    },
  ];

  it("replaces refs with ? and collects bind values in order", () => {
    const { sql, bindValues, errors } = resolveRefsToBindParams(
      "SELECT * FROM t WHERE id={{login.response.body.id}} AND tok={{login.response.body.token}}",
      blocks,
      100,
    );
    expect(errors).toHaveLength(0);
    expect(sql).toBe("SELECT * FROM t WHERE id=? AND tok=?");
    expect(bindValues).toEqual([42, "abc"]);
  });

  it("coerces numeric strings to numbers", () => {
    const { bindValues } = resolveRefsToBindParams(
      "SELECT {{login.response.body.id}}",
      blocks,
      100,
    );
    expect(bindValues[0]).toBe(42);
    expect(typeof bindValues[0]).toBe("number");
  });

  it("coerces true/false/null strings to JS literals", () => {
    const b: BlockContext[] = [
      {
        alias: "flags",
        blockType: "http",
        pos: 5,
        content: "",
        cachedResult: {
          status: "success",
          response: JSON.stringify({
            body: { on: true, off: false, empty: null },
          }),
        },
      },
    ];
    const { bindValues } = resolveRefsToBindParams(
      "SELECT {{flags.response.body.on}}, {{flags.response.body.off}}, {{flags.response.body.empty}}",
      b,
      100,
    );
    expect(bindValues).toEqual([true, false, null]);
  });

  it("resolves env vars into bind values", () => {
    const { sql, bindValues, errors } = resolveRefsToBindParams(
      "WHERE region={{REGION}}",
      [],
      100,
      { REGION: "us-east" },
    );
    expect(errors).toHaveLength(0);
    expect(sql).toBe("WHERE region=?");
    expect(bindValues).toEqual(["us-east"]);
  });

  it("keeps original ref and collects error when resolution fails", () => {
    const { sql, bindValues, errors } = resolveRefsToBindParams(
      "SELECT {{unknown.x}}",
      blocks,
      100,
    );
    expect(sql).toBe("SELECT {{unknown.x}}");
    expect(bindValues).toHaveLength(0);
    expect(errors.length).toBeGreaterThan(0);
  });

  it("handles queries without any references", () => {
    const { sql, bindValues, errors } = resolveRefsToBindParams(
      "SELECT * FROM t",
      blocks,
      100,
    );
    expect(sql).toBe("SELECT * FROM t");
    expect(bindValues).toEqual([]);
    expect(errors).toHaveLength(0);
  });

  it("preserves ref order when refs appear multiple times in one query", () => {
    const { bindValues } = resolveRefsToBindParams(
      "SELECT {{login.response.body.id}}, {{login.response.body.token}}, {{login.response.body.id}}",
      blocks,
      100,
    );
    expect(bindValues).toEqual([42, "abc", 42]);
  });
});
