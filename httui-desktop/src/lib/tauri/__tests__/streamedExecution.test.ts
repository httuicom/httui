import { describe, it, expect, beforeEach } from "vitest";
import {
  normalizeHttpResponse,
  applyConnectionOverride,
} from "../streamedExecution";
import { useConnectionSessionOverrideStore } from "@/stores/connectionSessionOverride";

describe("normalizeHttpResponse", () => {
  it("accepts the new full shape verbatim", () => {
    const raw = {
      status_code: 200,
      status_text: "OK",
      headers: { "content-type": "application/json" },
      body: { hello: "world" },
      size_bytes: 17,
      elapsed_ms: 42,
      timing: { total_ms: 42, dns_ms: 5 },
      cookies: [
        {
          name: "sid",
          value: "abc",
          secure: true,
          http_only: false,
        },
      ],
    };
    const out = normalizeHttpResponse(raw);
    expect(out.status_code).toBe(200);
    expect(out.body).toEqual({ hello: "world" });
    expect(out.timing.total_ms).toBe(42);
    expect(out.timing.dns_ms).toBe(5);
    expect(out.cookies).toHaveLength(1);
    expect(out.cookies[0].name).toBe("sid");
  });

  it("synthesizes timing from elapsed_ms when missing", () => {
    const raw = {
      status_code: 200,
      status_text: "OK",
      headers: {},
      body: "",
      size_bytes: 0,
      elapsed_ms: 123,
    };
    const out = normalizeHttpResponse(raw);
    expect(out.timing.total_ms).toBe(123);
    // V2 sub-fields stay nullish until isahc swap.
    expect(out.timing.dns_ms ?? null).toBeNull();
    expect(out.timing.connect_ms ?? null).toBeNull();
    expect(out.timing.connection_reused).toBe(false);
    expect(out.cookies).toEqual([]);
  });

  it("falls back to duration_ms (legacy cached shape)", () => {
    const raw = {
      status_code: 404,
      status_text: "Not Found",
      headers: {},
      body: "Not Found",
      size_bytes: 9,
      duration_ms: 18,
    };
    const out = normalizeHttpResponse(raw);
    expect(out.elapsed_ms).toBe(18);
    expect(out.timing.total_ms).toBe(18);
  });

  it("returns sane defaults for completely empty input", () => {
    const out = normalizeHttpResponse({});
    expect(out.status_code).toBe(0);
    expect(out.status_text).toBe("");
    expect(out.headers).toEqual({});
    expect(out.body).toBeUndefined();
    expect(out.size_bytes).toBe(0);
    expect(out.elapsed_ms).toBe(0);
    expect(out.timing.total_ms).toBe(0);
    expect(out.timing.connection_reused).toBe(false);
    expect(out.cookies).toEqual([]);
  });

  it("ignores invalid types instead of throwing", () => {
    const out = normalizeHttpResponse({
      status_code: "200",
      headers: "not an object",
      cookies: "nope",
      timing: 42,
    });
    expect(out.status_code).toBe(0);
    expect(out.headers).toEqual({});
    expect(out.cookies).toEqual([]);
    expect(out.timing.total_ms).toBe(0);
    expect(out.timing.connection_reused).toBe(false);
  });

  it("handles non-object roots", () => {
    expect(normalizeHttpResponse(null).status_code).toBe(0);
    expect(normalizeHttpResponse(undefined).status_code).toBe(0);
    expect(normalizeHttpResponse("string").status_code).toBe(0);
  });

  it("preserves Onda 4 timing fields (ttfb_ms + connection_reused)", () => {
    const raw = {
      status_code: 200,
      status_text: "OK",
      headers: {},
      body: "ok",
      size_bytes: 2,
      elapsed_ms: 150,
      timing: {
        total_ms: 150,
        ttfb_ms: 42,
        connection_reused: false,
      },
      cookies: [],
    };
    const out = normalizeHttpResponse(raw);
    expect(out.timing.ttfb_ms).toBe(42);
    expect(out.timing.connection_reused).toBe(false);
    // V2 sub-fields stay nullish.
    expect(out.timing.dns_ms ?? null).toBeNull();
    expect(out.timing.connect_ms ?? null).toBeNull();
    expect(out.timing.tls_ms ?? null).toBeNull();
  });

  it("defaults connection_reused to false for legacy cached shapes", () => {
    // Pre-Onda-4 cached responses don't carry `connection_reused`. The
    // normalizer must fill it in so consumers always see a boolean.
    const raw = {
      status_code: 200,
      status_text: "OK",
      headers: {},
      body: "ok",
      size_bytes: 2,
      elapsed_ms: 50,
      timing: { total_ms: 50 },
      cookies: [],
    };
    const out = normalizeHttpResponse(raw);
    expect(out.timing.connection_reused).toBe(false);
    expect(typeof out.timing.connection_reused).toBe("boolean");
  });
});

describe("applyConnectionOverride", () => {
  beforeEach(() => {
    useConnectionSessionOverrideStore.setState({ overrides: {} });
  });

  it("returns params unchanged when no override is set", () => {
    const params = { connection_id: "c1", query: "SELECT 1" };
    expect(applyConnectionOverride(params)).toBe(params);
  });

  it("returns params unchanged when connection_id is missing/blank", () => {
    useConnectionSessionOverrideStore
      .getState()
      .setOverride("c1", { host: "h" });
    const p1 = { query: "x" };
    expect(applyConnectionOverride(p1)).toBe(p1);
    const p2 = { connection_id: "", query: "x" };
    expect(applyConnectionOverride(p2)).toBe(p2);
  });

  it("merges host + port override into a fresh params object", () => {
    useConnectionSessionOverrideStore
      .getState()
      .setOverride("c1", { host: "db.staging", port: 5599 });
    const params = { connection_id: "c1", query: "SELECT 1" };
    const out = applyConnectionOverride(params);
    expect(out).not.toBe(params);
    expect(out).toMatchObject({
      connection_id: "c1",
      query: "SELECT 1",
      session_host_override: "db.staging",
      session_port_override: 5599,
    });
    // Original is not mutated.
    expect(params).not.toHaveProperty("session_host_override");
  });

  it("includes only the field that was overridden", () => {
    useConnectionSessionOverrideStore
      .getState()
      .setOverride("c1", { port: 6000 });
    const out = applyConnectionOverride({ connection_id: "c1" });
    expect(out.session_port_override).toBe(6000);
    expect(out).not.toHaveProperty("session_host_override");
  });

  it("does not apply another connection's override", () => {
    useConnectionSessionOverrideStore
      .getState()
      .setOverride("other", { host: "h" });
    const params = { connection_id: "c1" };
    expect(applyConnectionOverride(params)).toBe(params);
  });
});
