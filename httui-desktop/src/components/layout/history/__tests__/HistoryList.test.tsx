import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import {
  HistoryList,
  formatElapsed,
  formatRelative,
  hasPlan,
  label,
  outcomeTone,
} from "@/components/layout/history/HistoryList";
import type { HistoryEntry } from "@/lib/tauri/commands";
import { renderWithProviders, screen } from "@/test/render";

const FIXED_NOW = new Date("2026-04-30T16:00:00Z").getTime();

function entry(over: Partial<HistoryEntry> = {}): HistoryEntry {
  return {
    id: 1,
    file_path: "runbook.md",
    block_alias: "fetchUser",
    method: "GET",
    url_canonical: "https://api.example.com/users/1",
    status: 200,
    request_size: null,
    response_size: 12,
    elapsed_ms: 120,
    outcome: "ok",
    ran_at: new Date(FIXED_NOW - 5 * 1000).toISOString(),
    ...over,
  };
}

describe("HistoryList", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(FIXED_NOW);
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders the empty state when entries is empty", () => {
    renderWithProviders(<HistoryList entries={[]} />);
    expect(screen.getByTestId("history-empty")).toBeInTheDocument();
    expect(screen.queryByTestId("history-list")).not.toBeInTheDocument();
  });

  it("renders one row per entry with id + tone attrs", () => {
    renderWithProviders(
      <HistoryList
        entries={[
          entry({ id: 1 }),
          entry({ id: 2, status: 500, outcome: "ok" }),
          entry({ id: 3, status: 404, outcome: "ok" }),
        ]}
      />,
    );
    const rows = screen.getAllByTestId("history-row");
    expect(rows).toHaveLength(3);
    expect(rows[0]).toHaveAttribute("data-tone", "ok");
    expect(rows[1]).toHaveAttribute("data-tone", "err");
    expect(rows[2]).toHaveAttribute("data-tone", "warn");
  });

  it("renders rows as plain divs without onSelect", () => {
    renderWithProviders(<HistoryList entries={[entry()]} />);
    expect(screen.getByTestId("history-row").tagName).toBe("DIV");
  });

  it("renders rows as buttons + fires onSelect with the entry", async () => {
    const onSelect = vi.fn();
    const e = entry({ id: 42 });
    renderWithProviders(<HistoryList entries={[e]} onSelect={onSelect} />);
    const row = screen.getByTestId("history-row");
    expect(row.tagName).toBe("BUTTON");
    vi.useRealTimers();
    await userEvent.click(row);
    expect(onSelect).toHaveBeenCalledWith(e);
  });

  it("uses alias when present, else METHOD + URL", () => {
    expect(label(entry({ block_alias: "fetchUser" }))).toBe("fetchUser");
    expect(
      label(entry({ block_alias: "", method: "POST", url_canonical: "/x" })),
    ).toBe("POST /x");
    expect(
      label(entry({ block_alias: "  ", method: "PUT", url_canonical: "/y" })),
    ).toBe("PUT /y");
  });

  it("hides status text when entry.status is null", () => {
    renderWithProviders(<HistoryList entries={[entry({ status: null })]} />);
    const row = screen.getByTestId("history-row");
    expect(row.textContent).not.toMatch(/^200/);
  });

  it("hides elapsed when null", () => {
    renderWithProviders(
      <HistoryList entries={[entry({ elapsed_ms: null })]} />,
    );
    const row = screen.getByTestId("history-row");
    expect(row.textContent).not.toMatch(/ms/);
  });

  // Story 05 task 2 — EXPLAIN plan chip

  it("shows the plan chip when entry.plan is set", () => {
    renderWithProviders(
      <HistoryList
        entries={[entry({ plan: '[{"Plan":{"Node Type":"Seq Scan"}}]' })]}
      />,
    );
    expect(screen.getByTestId("history-row-plan")).toBeInTheDocument();
    expect(screen.getByTestId("history-row-plan").textContent).toContain(
      "plan",
    );
  });

  it("hides the plan chip when entry.plan is undefined", () => {
    renderWithProviders(<HistoryList entries={[entry()]} />);
    expect(screen.queryByTestId("history-row-plan")).not.toBeInTheDocument();
  });

  it("hides the plan chip when entry.plan is empty / whitespace", () => {
    renderWithProviders(<HistoryList entries={[entry({ plan: "   " })]} />);
    expect(screen.queryByTestId("history-row-plan")).not.toBeInTheDocument();
  });
});

describe("hasPlan", () => {
  it("returns true for a non-empty plan string", () => {
    expect(hasPlan(entry({ plan: '{"x":1}' }))).toBe(true);
  });
  it("returns false when plan is undefined / null / missing", () => {
    expect(hasPlan(entry())).toBe(false);
  });
  it("returns false for an empty / whitespace plan", () => {
    expect(hasPlan(entry({ plan: "" }))).toBe(false);
    expect(hasPlan(entry({ plan: "   " }))).toBe(false);
    expect(hasPlan(entry({ plan: "\t\n" }))).toBe(false);
  });
});

describe("outcomeTone", () => {
  it("classifies error / cancelled as err", () => {
    expect(outcomeTone(entry({ outcome: "error" }))).toBe("err");
    expect(outcomeTone(entry({ outcome: "cancelled" }))).toBe("err");
  });

  it("classifies 5xx as err, 4xx as warn, 2xx as ok", () => {
    expect(outcomeTone(entry({ status: 500 }))).toBe("err");
    expect(outcomeTone(entry({ status: 503 }))).toBe("err");
    expect(outcomeTone(entry({ status: 404 }))).toBe("warn");
    expect(outcomeTone(entry({ status: 401 }))).toBe("warn");
    expect(outcomeTone(entry({ status: 200 }))).toBe("ok");
    expect(outcomeTone(entry({ status: 204 }))).toBe("ok");
  });

  it("falls back to muted for unknown outcomes / 1xx / 3xx", () => {
    expect(outcomeTone(entry({ outcome: "weird" }))).toBe("muted");
    expect(outcomeTone(entry({ status: 301 }))).toBe("muted");
    expect(outcomeTone(entry({ status: null }))).toBe("muted");
  });
});

describe("formatRelative", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(FIXED_NOW);
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("returns seconds under 60", () => {
    expect(formatRelative(FIXED_NOW - 5 * 1000)).toBe("5s");
  });
  it("returns minutes under 60", () => {
    expect(formatRelative(FIXED_NOW - 90 * 1000)).toBe("1m");
  });
  it("returns hours under 24", () => {
    expect(formatRelative(FIXED_NOW - 3 * 3600 * 1000)).toBe("3h");
  });
  it("returns days otherwise", () => {
    expect(formatRelative(FIXED_NOW - 5 * 86400 * 1000)).toBe("5d");
  });
  it('returns "now" for future timestamps', () => {
    expect(formatRelative(FIXED_NOW + 5000)).toBe("now");
  });
  it("returns dash for unparseable input", () => {
    expect(formatRelative("not a date")).toBe("—");
  });
});

describe("formatElapsed", () => {
  it("renders ms under 1000", () => {
    expect(formatElapsed(0)).toBe("0ms");
    expect(formatElapsed(120)).toBe("120ms");
    expect(formatElapsed(999)).toBe("999ms");
  });
  it("renders s under 60_000 with one decimal", () => {
    expect(formatElapsed(1500)).toBe("1.5s");
    expect(formatElapsed(59_999)).toBe("60.0s");
  });
  it("renders m above 60_000", () => {
    expect(formatElapsed(120_000)).toBe("2m");
    expect(formatElapsed(3 * 60_000)).toBe("3m");
  });
});
