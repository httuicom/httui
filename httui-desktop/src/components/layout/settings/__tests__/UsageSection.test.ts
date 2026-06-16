import { describe, expect, it } from "vitest";

import { pivotFeatureUsageByDay } from "../UsageSection";
import type { FeatureUsage } from "@/lib/tauri/telemetry";

describe("pivotFeatureUsageByDay", () => {
  it("collapses rows into one entry per day with a column per feature", () => {
    const rows: FeatureUsage[] = [
      { date: "2026-01-10", feature: "http_block_run", count: 3 },
      { date: "2026-01-10", feature: "db_block_run", count: 2 },
      { date: "2026-01-11", feature: "http_block_run", count: 1 },
    ];
    expect(pivotFeatureUsageByDay(rows)).toEqual([
      { date: "2026-01-10", http: 3, db: 2 },
      { date: "2026-01-11", http: 1, db: 0 },
    ]);
  });

  it("sorts days ascending regardless of input order", () => {
    const rows: FeatureUsage[] = [
      { date: "2026-01-12", feature: "db_block_run", count: 1 },
      { date: "2026-01-09", feature: "http_block_run", count: 1 },
    ];
    expect(pivotFeatureUsageByDay(rows).map((d) => d.date)).toEqual([
      "2026-01-09",
      "2026-01-12",
    ]);
  });

  it("ignores unknown feature names", () => {
    const rows: FeatureUsage[] = [
      { date: "2026-01-10", feature: "mystery_feature", count: 9 },
      { date: "2026-01-10", feature: "http_block_run", count: 1 },
    ];
    expect(pivotFeatureUsageByDay(rows)).toEqual([
      { date: "2026-01-10", http: 1, db: 0 },
    ]);
  });

  it("returns an empty array for no rows", () => {
    expect(pivotFeatureUsageByDay([])).toEqual([]);
  });
});
