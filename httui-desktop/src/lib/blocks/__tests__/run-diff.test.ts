import { describe, expect, it } from "vitest";

import {
  diffHeaders,
  diffJson,
  diffRuns,
  type RunSnapshot,
} from "@/lib/blocks/run-diff";

describe("diffJson", () => {
  it("returns empty when scalars are equal", () => {
    expect(diffJson(1, 1)).toEqual([]);
    expect(diffJson("x", "x")).toEqual([]);
    expect(diffJson(null, null)).toEqual([]);
  });

  it("returns a change entry when scalars differ", () => {
    expect(diffJson(1, 2)).toEqual([
      { path: "$", op: "change", before: 1, after: 2 },
    ]);
  });

  it("flags type mismatches as a change entry at the path", () => {
    expect(diffJson("1", 1)).toEqual([
      { path: "$", op: "change", before: "1", after: 1 },
    ]);
  });

  it("emits add when before is undefined and after is set", () => {
    expect(diffJson(undefined, 5)).toEqual([
      { path: "$", op: "add", after: 5 },
    ]);
  });

  it("emits remove when before is set and after is undefined", () => {
    expect(diffJson(5, undefined)).toEqual([
      { path: "$", op: "remove", before: 5 },
    ]);
  });

  it("descends into objects with dot-path labels", () => {
    const before = { user: { id: 1, name: "alice" } };
    const after = { user: { id: 1, name: "bob" } };
    expect(diffJson(before, after)).toEqual([
      {
        path: "user.name",
        op: "change",
        before: "alice",
        after: "bob",
      },
    ]);
  });

  it("emits add for new object keys, remove for missing ones", () => {
    const before = { a: 1, b: 2 };
    const after = { a: 1, c: 3 };
    expect(diffJson(before, after)).toEqual([
      { path: "b", op: "remove", before: 2 },
      { path: "c", op: "add", after: 3 },
    ]);
  });

  it("walks arrays index-aligned with bracket paths", () => {
    expect(diffJson([1, 2, 3], [1, 9, 3])).toEqual([
      { path: "[1]", op: "change", before: 2, after: 9 },
    ]);
  });

  it("emits add/remove entries when array lengths differ", () => {
    expect(diffJson([1, 2], [1, 2, 3, 4])).toEqual([
      { path: "[2]", op: "add", after: 3 },
      { path: "[3]", op: "add", after: 4 },
    ]);
    expect(diffJson([1, 2, 3], [1])).toEqual([
      { path: "[1]", op: "remove", before: 2 },
      { path: "[2]", op: "remove", before: 3 },
    ]);
  });

  it("treats key reordering inside an object as no diff", () => {
    expect(diffJson({ a: 1, b: 2 }, { b: 2, a: 1 })).toEqual([]);
  });

  it("handles nested arrays of objects with full paths", () => {
    const before = { items: [{ id: 1 }, { id: 2 }] };
    const after = { items: [{ id: 1 }, { id: 9 }] };
    expect(diffJson(before, after)).toEqual([
      { path: "items[1].id", op: "change", before: 2, after: 9 },
    ]);
  });

  it("flags object-vs-array type mismatch as a single change entry", () => {
    expect(diffJson({ a: 1 }, [1])).toEqual([
      { path: "$", op: "change", before: { a: 1 }, after: [1] },
    ]);
  });
});

describe("diffHeaders", () => {
  it("flags equal entries with op=equal (so the UI can render the row)", () => {
    expect(
      diffHeaders(
        { "Content-Type": "application/json" },
        { "content-type": "application/json" },
      ),
    ).toEqual([
      {
        key: "content-type",
        op: "equal",
        before: "application/json",
        after: "application/json",
      },
    ]);
  });

  it("matches keys case-insensitively but echoes the after-side casing", () => {
    expect(diffHeaders({ "X-Trace": "a" }, { "x-trace": "b" })).toEqual([
      { key: "x-trace", op: "change", before: "a", after: "b" },
    ]);
  });

  it("emits add/remove for one-sided keys, sorted by lower-cased key", () => {
    expect(
      diffHeaders({ A: "1" }, { B: "2" }).map((h) => `${h.op}:${h.key}`),
    ).toEqual(["remove:A", "add:B"]);
  });

  it("returns rows sorted by lower-cased key", () => {
    expect(
      diffHeaders({ Z: "1", A: "2" }, { Z: "1", A: "2" }).map((h) => h.key),
    ).toEqual(["A", "Z"]);
  });

  it("treats undefined inputs as empty maps", () => {
    expect(diffHeaders()).toEqual([]);
    expect(diffHeaders({ A: "1" })).toEqual([
      { key: "A", op: "remove", before: "1" },
    ]);
  });
});

describe("diffRuns", () => {
  function snap(over: Partial<RunSnapshot> = {}): RunSnapshot {
    return {
      status: 200,
      headers: { "Content-Type": "application/json" },
      body: { id: 1 },
      time_ms: 100,
      ...over,
    };
  }

  it("flags status changes via status.changed", () => {
    const out = diffRuns(snap(), snap({ status: 500 }));
    expect(out.status).toEqual({ before: 200, after: 500, changed: true });
  });

  it("computes timing.deltaMs as after - before", () => {
    const out = diffRuns(snap({ time_ms: 100 }), snap({ time_ms: 250 }));
    expect(out.timing.deltaMs).toBe(150);
  });

  it("leaves timing.deltaMs undefined when one side is missing", () => {
    const out = diffRuns(snap({ time_ms: undefined }), snap());
    expect(out.timing.deltaMs).toBeUndefined();
  });

  it("walks headers + body using the lower-level helpers", () => {
    const out = diffRuns(snap({ body: { id: 1 } }), snap({ body: { id: 2 } }));
    expect(out.body).toEqual([
      { path: "id", op: "change", before: 1, after: 2 },
    ]);
  });

  it("skips body diff when either side exceeds 200 KB", () => {
    const big = snap({ size_bytes: 250 * 1024 });
    const out = diffRuns(big, snap());
    expect(out.bodyTruncated).toBe(true);
    expect(out.body).toEqual([]);
  });

  it("does not skip when neither side exceeds the cap", () => {
    const out = diffRuns(
      snap({ size_bytes: 1024 }),
      snap({ size_bytes: 1024, body: { id: 9 } }),
    );
    expect(out.bodyTruncated).toBe(false);
    expect(out.body.length).toBe(1);
  });

  it("returns empty diffs when the two runs are identical", () => {
    const a = snap();
    const b = snap();
    const out = diffRuns(a, b);
    expect(out.status.changed).toBe(false);
    expect(out.body).toEqual([]);
    expect(out.headers.every((h) => h.op === "equal")).toBe(true);
  });
});
