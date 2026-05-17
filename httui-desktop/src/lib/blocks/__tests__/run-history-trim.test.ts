import { describe, expect, it } from "vitest";

import type { HistoryEntry } from "@/lib/tauri/commands";

import { KEEP_PER_BLOCK, pickKeepN, trimToKeepN } from "../run-history-trim";

function entry(over: Partial<HistoryEntry> = {}): HistoryEntry {
  return {
    id: 1,
    file_path: "a.md",
    block_alias: "x",
    method: "GET",
    url_canonical: "https://api/users",
    status: 200,
    request_size: null,
    response_size: 100,
    elapsed_ms: 50,
    outcome: "success",
    ran_at: "2026-05-02T12:00:00Z",
    ...over,
  };
}

describe("pickKeepN", () => {
  it("returns empty kept + all dropped when n <= 0", () => {
    const result = pickKeepN([entry()], 0);
    expect(result.kept).toHaveLength(0);
    expect(result.dropped).toHaveLength(1);
  });

  it("keeps everything when there are fewer entries than n", () => {
    const result = pickKeepN(
      [entry({ id: 1 }), entry({ id: 2 })],
      KEEP_PER_BLOCK,
    );
    expect(result.kept).toHaveLength(2);
    expect(result.dropped).toHaveLength(0);
  });

  it("keeps the newest n per (file_path, alias) group", () => {
    const entries: HistoryEntry[] = [];
    for (let i = 0; i < 12; i++) {
      entries.push(
        entry({
          id: i,
          ran_at: `2026-05-02T12:${String(i).padStart(2, "0")}:00Z`,
        }),
      );
    }
    const result = pickKeepN(entries, 10);
    expect(result.kept).toHaveLength(10);
    expect(result.dropped).toHaveLength(2);
    // Survivors must include the two newest (id 11 and 10).
    expect(result.kept.map((e) => e.id)).toContain(11);
    expect(result.kept.map((e) => e.id)).toContain(10);
    // Dropped must be the two oldest.
    expect(result.dropped.map((e) => e.id).sort()).toEqual([0, 1]);
  });

  it("trims per-group independently", () => {
    const entries: HistoryEntry[] = [];
    // Group A: 3 entries
    for (let i = 0; i < 3; i++) {
      entries.push(
        entry({
          id: i,
          file_path: "a.md",
          block_alias: "x",
          ran_at: `2026-05-02T12:0${i}:00Z`,
        }),
      );
    }
    // Group B: 5 entries
    for (let i = 0; i < 5; i++) {
      entries.push(
        entry({
          id: 100 + i,
          file_path: "b.md",
          block_alias: "y",
          ran_at: `2026-05-02T12:0${i}:00Z`,
        }),
      );
    }
    const result = pickKeepN(entries, 2);
    // Group A: keep 2, drop 1. Group B: keep 2, drop 3.
    expect(result.kept).toHaveLength(4);
    expect(result.dropped).toHaveLength(4);
    // Newest in group A is id=2 (ran_at 12:02); in group B is id=104.
    expect(result.kept.map((e) => e.id)).toContain(2);
    expect(result.kept.map((e) => e.id)).toContain(104);
  });

  it("treats different aliases as different groups even on the same file", () => {
    const entries: HistoryEntry[] = [
      entry({ id: 1, block_alias: "x" }),
      entry({ id: 2, block_alias: "x" }),
      entry({ id: 3, block_alias: "y" }),
      entry({ id: 4, block_alias: "y" }),
    ];
    const result = pickKeepN(entries, 1);
    expect(result.kept).toHaveLength(2);
    expect(result.dropped).toHaveLength(2);
  });

  it("breaks ran_at ties by id desc (newer auto-id wins)", () => {
    const entries: HistoryEntry[] = [
      entry({ id: 100, ran_at: "2026-05-02T12:00:00Z" }),
      entry({ id: 101, ran_at: "2026-05-02T12:00:00Z" }),
    ];
    const result = pickKeepN(entries, 1);
    expect(result.kept[0]!.id).toBe(101);
    expect(result.dropped[0]!.id).toBe(100);
  });

  it("returns dropped entries in input order within each group", () => {
    // The dropped array isn't strictly ordered; consumers just feed
    // them to a cleanup routine. But the test guards against the
    // function shuffling them more than necessary.
    const entries: HistoryEntry[] = [];
    for (let i = 0; i < 5; i++) {
      entries.push(
        entry({
          id: i,
          ran_at: `2026-05-02T12:0${i}:00Z`,
        }),
      );
    }
    const result = pickKeepN(entries, 2);
    // Dropped contains the 3 oldest (ids 0, 1, 2). Order within
    // dropped reflects the descending sort: id 2 first (newest of
    // the dropped trio), then id 1, then id 0.
    expect(result.dropped.map((e) => e.id)).toEqual([2, 1, 0]);
  });

  it("defaults n to KEEP_PER_BLOCK", () => {
    const entries: HistoryEntry[] = [];
    for (let i = 0; i < KEEP_PER_BLOCK + 3; i++) {
      entries.push(
        entry({
          id: i,
          ran_at: `2026-05-02T12:${String(i).padStart(2, "0")}:00Z`,
        }),
      );
    }
    const result = pickKeepN(entries);
    expect(result.kept).toHaveLength(KEEP_PER_BLOCK);
  });

  it("does not mutate the input array", () => {
    const entries: HistoryEntry[] = [entry({ id: 1 }), entry({ id: 2 })];
    const before = entries.map((e) => e.id);
    pickKeepN(entries, 1);
    expect(entries.map((e) => e.id)).toEqual(before);
  });
});

describe("trimToKeepN", () => {
  it("returns just the kept entries", () => {
    const entries: HistoryEntry[] = [];
    for (let i = 0; i < 3; i++) {
      entries.push(
        entry({
          id: i,
          ran_at: `2026-05-02T12:0${i}:00Z`,
        }),
      );
    }
    const trimmed = trimToKeepN(entries, 2);
    expect(trimmed).toHaveLength(2);
    expect(trimmed.map((e) => e.id).sort()).toEqual([1, 2]);
  });
});
