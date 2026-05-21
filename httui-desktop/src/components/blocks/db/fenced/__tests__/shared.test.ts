import { describe, it, expect } from "vitest";
import {
  formatRelativeTime,
  isPlainObject,
  type ExecutionState,
} from "../shared";

describe("formatRelativeTime", () => {
  const NOW = new Date("2026-05-19T12:00:00Z").getTime();

  it("returns 'just now' for events < 5s ago", () => {
    expect(formatRelativeTime(NOW - 0, NOW)).toBe("just now");
    expect(formatRelativeTime(NOW - 4_999, NOW)).toBe("just now");
  });

  it("clamps negative deltas to 'just now' (clock skew safety net)", () => {
    expect(formatRelativeTime(NOW + 10_000, NOW)).toBe("just now");
  });

  it("returns 'Xs ago' for events under 60s", () => {
    expect(formatRelativeTime(NOW - 30_000, NOW)).toBe("30s ago");
    expect(formatRelativeTime(NOW - 59_999, NOW)).toBe("59s ago");
  });

  it("returns 'Xm ago' for events under 60m", () => {
    expect(formatRelativeTime(NOW - 5 * 60_000, NOW)).toBe("5m ago");
    expect(formatRelativeTime(NOW - 59 * 60_000, NOW)).toBe("59m ago");
  });

  it("returns 'Xh ago' for events under 24h", () => {
    expect(formatRelativeTime(NOW - 3 * 3_600_000, NOW)).toBe("3h ago");
    expect(formatRelativeTime(NOW - 23 * 3_600_000, NOW)).toBe("23h ago");
  });

  it("returns 'Xd ago' for events under 7d", () => {
    expect(formatRelativeTime(NOW - 24 * 3_600_000, NOW)).toBe("1d ago");
    expect(formatRelativeTime(NOW - 6 * 24 * 3_600_000, NOW)).toBe("6d ago");
  });

  it("falls back to an ISO date (YYYY-MM-DD) for >= 7d ago", () => {
    expect(formatRelativeTime(NOW - 7 * 24 * 3_600_000, NOW)).toBe(
      "2026-05-12",
    );
    expect(formatRelativeTime(NOW - 30 * 24 * 3_600_000, NOW)).toBe(
      "2026-04-19",
    );
  });
});

describe("isPlainObject", () => {
  it("returns true for plain object literals", () => {
    expect(isPlainObject({})).toBe(true);
    expect(isPlainObject({ a: 1 })).toBe(true);
  });

  it("returns false for arrays", () => {
    expect(isPlainObject([])).toBe(false);
    expect(isPlainObject([1, 2, 3])).toBe(false);
  });

  it("returns false for null / undefined", () => {
    expect(isPlainObject(null)).toBe(false);
    expect(isPlainObject(undefined)).toBe(false);
  });

  it("returns false for primitives", () => {
    expect(isPlainObject(42)).toBe(false);
    expect(isPlainObject("str")).toBe(false);
    expect(isPlainObject(true)).toBe(false);
  });

  it("narrows the type so the value can be indexed as Record", () => {
    const v: unknown = { foo: "bar" };
    if (isPlainObject(v)) {
      // Compile-time: v is now `Record<string, unknown>`.
      expect(v.foo).toBe("bar");
    }
  });
});

describe("ExecutionState type re-export", () => {
  it("accepts the 5 documented values", () => {
    // Pure type assertion — proves the re-export resolves and the
    // type carries the expected union literals.
    const states: ExecutionState[] = [
      "idle",
      "running",
      "success",
      "error",
      "cancelled",
    ];
    expect(states).toHaveLength(5);
  });
});
