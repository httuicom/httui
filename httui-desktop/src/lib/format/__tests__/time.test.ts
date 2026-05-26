import { describe, it, expect } from "vitest";

import { formatElapsed, formatDurationCompact } from "@/lib/format/time";

describe("formatElapsed", () => {
  it("renders ms under 1000", () => {
    expect(formatElapsed(0)).toBe("0ms");
    expect(formatElapsed(120)).toBe("120ms");
    expect(formatElapsed(999)).toBe("999ms");
  });
  it("renders two-decimal seconds at/above 1000ms", () => {
    expect(formatElapsed(1000)).toBe("1.00s");
    expect(formatElapsed(1500)).toBe("1.50s");
    expect(formatElapsed(5200)).toBe("5.20s");
    expect(formatElapsed(90_000)).toBe("90.00s");
  });
});

// These assertions mirror the previous HistoryList.formatElapsed tests
// verbatim — proves the relocated/renamed formatter preserves behavior.
describe("formatDurationCompact", () => {
  it("renders ms under 1000", () => {
    expect(formatDurationCompact(0)).toBe("0ms");
    expect(formatDurationCompact(120)).toBe("120ms");
    expect(formatDurationCompact(999)).toBe("999ms");
  });
  it("renders s under 60_000 with one decimal", () => {
    expect(formatDurationCompact(1500)).toBe("1.5s");
    expect(formatDurationCompact(59_999)).toBe("60.0s");
  });
  it("renders m above 60_000", () => {
    expect(formatDurationCompact(120_000)).toBe("2m");
    expect(formatDurationCompact(3 * 60_000)).toBe("3m");
  });
});
