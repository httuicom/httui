import { describe, expect, it } from "vitest";

import { serializeExplainPlan } from "@/lib/blocks/serialize-explain-plan";

describe("serializeExplainPlan", () => {
  it("returns undefined for null", () => {
    expect(serializeExplainPlan(null)).toBeUndefined();
  });

  it("returns undefined for undefined", () => {
    expect(serializeExplainPlan(undefined)).toBeUndefined();
  });

  it("passes a string through verbatim (truncated-fallback shape)", () => {
    // The 200 KB cap path stores the truncated text as Value::String
    // on the Rust side. The wire delivers a plain string; we keep
    // it as-is so the consumer sees the truncation marker.
    const truncated = "[".padEnd(190_000, "x") + "[explain payload truncated]";
    expect(serializeExplainPlan(truncated)).toBe(truncated);
  });

  it("JSON.stringify's a Postgres-shape parsed plan (array)", () => {
    const plan = [{ Plan: { "Node Type": "Seq Scan", "Total Cost": 12.5 } }];
    const s = serializeExplainPlan(plan);
    expect(s).toBe(JSON.stringify(plan));
    // Round-trips cleanly so the consumer can JSON.parse on read.
    expect(JSON.parse(s!)).toEqual(plan);
  });

  it("JSON.stringify's a MySQL-shape parsed plan (object)", () => {
    const plan = {
      query_block: { select_id: 1, table: { access_type: "ref" } },
    };
    const s = serializeExplainPlan(plan);
    expect(JSON.parse(s!)).toEqual(plan);
  });

  it("handles primitive non-string values (defensive)", () => {
    // The executor never emits these, but if a future driver
    // does, the helper shouldn't drop the value silently.
    expect(serializeExplainPlan(42)).toBe("42");
    expect(serializeExplainPlan(true)).toBe("true");
  });

  it("returns undefined on circular references (no crash)", () => {
    const plan: Record<string, unknown> = { a: 1 };
    plan.self = plan;
    expect(serializeExplainPlan(plan)).toBeUndefined();
  });

  it("returns the string value '\"\"' for an empty-string plan", () => {
    // An empty string from the truncated path is still meaningful
    // (someone hit the cap with all-whitespace?); pass it through.
    expect(serializeExplainPlan("")).toBe("");
  });

  it("preserves nested arrays inside the parsed plan shape", () => {
    const plan = {
      query_block: {
        nested_loop: [{ table: { access_type: "ALL" } }],
      },
    };
    const s = serializeExplainPlan(plan);
    expect(JSON.parse(s!)).toEqual(plan);
  });
});
