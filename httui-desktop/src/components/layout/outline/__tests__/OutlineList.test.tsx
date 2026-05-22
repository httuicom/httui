import { describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { OutlineList } from "@/components/layout/outline/OutlineList";
import type { OutlineEntry } from "@/lib/blocks/outline";
import { renderWithProviders, screen } from "@/test/render";

const SAMPLE: OutlineEntry[] = [
  { level: 1, text: "Sanidade", line: 3, offset: 10 },
  { level: 2, text: "Health check", line: 8, offset: 50 },
  { level: 1, text: "Rollout", line: 20, offset: 200 },
  { level: 2, text: "PATCH", line: 25, offset: 260 },
  { level: 3, text: "Validation", line: 30, offset: 320 },
];

describe("OutlineList", () => {
  it("renders the empty state when entries is empty", () => {
    renderWithProviders(<OutlineList entries={[]} />);
    expect(screen.getByTestId("outline-empty")).toBeInTheDocument();
    expect(screen.queryByTestId("outline-list")).not.toBeInTheDocument();
  });

  it("renders one row per entry with level + line attrs", () => {
    renderWithProviders(<OutlineList entries={SAMPLE} />);
    const rows = screen.getAllByTestId("outline-row");
    expect(rows).toHaveLength(5);
    expect(rows[0]).toHaveAttribute("data-level", "1");
    expect(rows[0]).toHaveAttribute("data-line", "3");
    expect(rows[1]).toHaveAttribute("data-level", "2");
    expect(rows[4]).toHaveAttribute("data-level", "3");
  });

  it("includes positional numbering by default", () => {
    renderWithProviders(<OutlineList entries={SAMPLE} />);
    const list = screen.getByTestId("outline-list");
    expect(list.textContent).toMatch(/1\..*Sanidade/);
    expect(list.textContent).toMatch(/3\..*Rollout/);
  });

  it("disables numbering when numbered=false", () => {
    renderWithProviders(<OutlineList entries={SAMPLE} numbered={false} />);
    const list = screen.getByTestId("outline-list");
    expect(list.textContent).not.toMatch(/^1\./);
  });

  it("renders rows as plain divs without onSelect", () => {
    renderWithProviders(<OutlineList entries={SAMPLE} />);
    const rows = screen.getAllByTestId("outline-row");
    expect(rows[0].tagName).toBe("DIV");
  });

  it("renders rows as buttons + fires onSelect with the entry", async () => {
    const onSelect = vi.fn();
    renderWithProviders(<OutlineList entries={SAMPLE} onSelect={onSelect} />);
    const rows = screen.getAllByTestId("outline-row");
    expect(rows[0].tagName).toBe("BUTTON");
    await userEvent.click(rows[2]);
    expect(onSelect).toHaveBeenCalledWith(SAMPLE[2]);
  });

  it("highlights the entry whose line is largest <= activeLine", () => {
    renderWithProviders(<OutlineList entries={SAMPLE} activeLine={22} />);
    const rows = screen.getAllByTestId("outline-row");
    // activeLine=22 sits between Rollout (line 20) and PATCH (line 25)
    // → Rollout (idx 2) highlights.
    expect(rows[0]).toHaveAttribute("data-active", "false");
    expect(rows[1]).toHaveAttribute("data-active", "false");
    expect(rows[2]).toHaveAttribute("data-active", "true");
    expect(rows[3]).toHaveAttribute("data-active", "false");
    expect(rows[4]).toHaveAttribute("data-active", "false");
  });

  it("highlights nothing when activeLine is before every heading", () => {
    renderWithProviders(<OutlineList entries={SAMPLE} activeLine={1} />);
    const rows = screen.getAllByTestId("outline-row");
    rows.forEach((r) => expect(r).toHaveAttribute("data-active", "false"));
  });

  it("highlights the last entry when activeLine is far past the end", () => {
    renderWithProviders(<OutlineList entries={SAMPLE} activeLine={9999} />);
    const rows = screen.getAllByTestId("outline-row");
    expect(rows[rows.length - 1]).toHaveAttribute("data-active", "true");
  });

  it("highlights nothing when activeLine is omitted", () => {
    renderWithProviders(<OutlineList entries={SAMPLE} />);
    const rows = screen.getAllByTestId("outline-row");
    rows.forEach((r) => expect(r).toHaveAttribute("data-active", "false"));
  });

  it("indent class scales with level via inline padding", () => {
    renderWithProviders(<OutlineList entries={SAMPLE} />);
    const rows = screen.getAllByTestId("outline-row");
    expect(rows[0]).toHaveStyle({ paddingLeft: "12px" }); // level 1
    expect(rows[1]).toHaveStyle({ paddingLeft: "24px" }); // level 2
    expect(rows[4]).toHaveStyle({ paddingLeft: "36px" }); // level 3
  });

  it("each entry has a title attribute carrying full heading text", () => {
    renderWithProviders(<OutlineList entries={SAMPLE} />);
    const rows = screen.getAllByTestId("outline-row");
    expect(rows[0].textContent).toContain("Sanidade");
    // Title is on the inner Text element — query by text.
    expect(screen.getByTitle("Sanidade")).toBeInTheDocument();
  });
});
