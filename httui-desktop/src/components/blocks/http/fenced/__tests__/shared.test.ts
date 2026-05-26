import { describe, it, expect, beforeAll, afterAll, vi } from "vitest";
import {
  METHOD_COLORS,
  MUTATION_METHODS,
  statusDotColor,
  formatBytes,
  bodyAsText,
  relativeTimeAgo,
} from "../shared";

describe("METHOD_COLORS", () => {
  it("maps every HTTP method to a Chakra color token", () => {
    expect(METHOD_COLORS.GET).toBe("green.500");
    expect(METHOD_COLORS.POST).toBe("blue.500");
    expect(METHOD_COLORS.PUT).toBe("orange.500");
    expect(METHOD_COLORS.PATCH).toBe("yellow.500");
    expect(METHOD_COLORS.DELETE).toBe("red.500");
    expect(METHOD_COLORS.HEAD).toBe("purple.500");
    expect(METHOD_COLORS.OPTIONS).toBe("gray.500");
  });
});

describe("MUTATION_METHODS", () => {
  it("contains the 4 destructive verbs and only those", () => {
    expect(MUTATION_METHODS.has("POST")).toBe(true);
    expect(MUTATION_METHODS.has("PUT")).toBe(true);
    expect(MUTATION_METHODS.has("PATCH")).toBe(true);
    expect(MUTATION_METHODS.has("DELETE")).toBe(true);
    expect(MUTATION_METHODS.has("GET")).toBe(false);
    expect(MUTATION_METHODS.has("HEAD")).toBe(false);
    expect(MUTATION_METHODS.has("OPTIONS")).toBe(false);
  });
});

describe("statusDotColor", () => {
  it("returns gray for null / undefined / 0", () => {
    expect(statusDotColor(null)).toBe("gray.400");
    expect(statusDotColor(undefined)).toBe("gray.400");
    expect(statusDotColor(0)).toBe("gray.400");
  });

  it("returns green for 2xx", () => {
    expect(statusDotColor(200)).toBe("green.500");
    expect(statusDotColor(204)).toBe("green.500");
    expect(statusDotColor(299)).toBe("green.500");
  });

  it("returns blue for 3xx", () => {
    expect(statusDotColor(300)).toBe("blue.500");
    expect(statusDotColor(301)).toBe("blue.500");
    expect(statusDotColor(399)).toBe("blue.500");
  });

  it("returns orange for 4xx", () => {
    expect(statusDotColor(400)).toBe("orange.500");
    expect(statusDotColor(404)).toBe("orange.500");
    expect(statusDotColor(499)).toBe("orange.500");
  });

  it("returns red for 5xx and above", () => {
    expect(statusDotColor(500)).toBe("red.500");
    expect(statusDotColor(503)).toBe("red.500");
    expect(statusDotColor(599)).toBe("red.500");
    expect(statusDotColor(600)).toBe("red.500");
  });

  it("returns gray for sub-200 (1xx informational)", () => {
    // 1xx is rare in REST; the function falls through to gray.400.
    expect(statusDotColor(100)).toBe("gray.400");
    expect(statusDotColor(199)).toBe("gray.400");
  });
});

describe("formatBytes", () => {
  it("formats values below 1 KB as plain bytes", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(1)).toBe("1 B");
    expect(formatBytes(1023)).toBe("1023 B");
  });

  it("formats kilobytes with 1 decimal place", () => {
    expect(formatBytes(1024)).toBe("1.0 KB");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(1024 * 1024 - 1)).toMatch(/KB$/);
  });

  it("formats megabytes with 2 decimal places", () => {
    expect(formatBytes(1024 * 1024)).toBe("1.00 MB");
    expect(formatBytes(1024 * 1024 * 2.5)).toBe("2.50 MB");
  });
});

describe("bodyAsText", () => {
  it("returns empty string for null / undefined", () => {
    expect(bodyAsText(null)).toBe("");
    expect(bodyAsText(undefined)).toBe("");
  });

  it("returns string bodies verbatim", () => {
    expect(bodyAsText("hello")).toBe("hello");
    expect(bodyAsText("")).toBe("");
  });

  it("recognises the base64-binary marker shape", () => {
    expect(bodyAsText({ encoding: "base64", data: "abc" })).toBe(
      "[binary content — base64 encoded]",
    );
  });

  it("pretty-prints JSON objects with 2-space indent", () => {
    expect(bodyAsText({ a: 1, b: [2, 3] })).toBe(
      `{\n  "a": 1,\n  "b": [\n    2,\n    3\n  ]\n}`,
    );
  });

  it("falls back to String() for non-stringifiable values", () => {
    // Circular reference defeats JSON.stringify — falls through to String().
    const circular: Record<string, unknown> = {};
    circular.self = circular;
    expect(bodyAsText(circular)).toBe("[object Object]");
  });
});

describe("relativeTimeAgo", () => {
  // Pin the clock so the assertions are deterministic.
  const FIXED_NOW = new Date("2026-05-19T12:00:00Z").getTime();

  beforeAll(() => {
    vi.useFakeTimers();
    vi.setSystemTime(FIXED_NOW);
  });
  afterAll(() => {
    vi.useRealTimers();
  });

  it("returns null for null input", () => {
    expect(relativeTimeAgo(null)).toBeNull();
  });

  it("returns 'just now' for events < 5s ago", () => {
    // The seconds are rounded, so 4_499ms rounds to 4 (< 5 → just now)
    // but 4_500ms rounds to 5 → "5s ago".
    expect(relativeTimeAgo(new Date(FIXED_NOW - 1_000))).toBe("just now");
    expect(relativeTimeAgo(new Date(FIXED_NOW - 4_499))).toBe("just now");
  });

  it("returns 'Xs ago' for events under 60s", () => {
    expect(relativeTimeAgo(new Date(FIXED_NOW - 30_000))).toBe("30s ago");
    expect(relativeTimeAgo(new Date(FIXED_NOW - 59_000))).toBe("59s ago");
  });

  it("returns 'Xm ago' for events under 60m", () => {
    expect(relativeTimeAgo(new Date(FIXED_NOW - 5 * 60_000))).toBe("5m ago");
    expect(relativeTimeAgo(new Date(FIXED_NOW - 59 * 60_000))).toBe("59m ago");
  });

  it("returns 'Xh ago' for events under 24h", () => {
    expect(relativeTimeAgo(new Date(FIXED_NOW - 3 * 3_600_000))).toBe("3h ago");
    expect(relativeTimeAgo(new Date(FIXED_NOW - 23 * 3_600_000))).toBe(
      "23h ago",
    );
  });

  it("returns 'Xd ago' for events >= 24h", () => {
    expect(relativeTimeAgo(new Date(FIXED_NOW - 24 * 3_600_000))).toBe(
      "1d ago",
    );
    expect(relativeTimeAgo(new Date(FIXED_NOW - 7 * 24 * 3_600_000))).toBe(
      "7d ago",
    );
  });
});
