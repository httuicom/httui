import { describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { DocHeaderShell } from "@/components/layout/docheader/DocHeaderShell";
import type { PreflightPillItem } from "@/components/blocks/preflight/PreflightPills";
import { renderWithProviders, screen } from "@/test/render";

function preflightItem(
  over: Partial<PreflightPillItem> = {},
): PreflightPillItem {
  return {
    id: "i1",
    label: "i1",
    result: { outcome: "pass" },
    ...over,
  };
}

describe("DocHeaderShell", () => {
  it("renders the card with the H1 derived from the filename when no frontmatter", () => {
    renderWithProviders(<DocHeaderShell filePath="notes/db.md" />);
    expect(screen.getByTestId("docheader-shell")).toBeInTheDocument();
    expect(screen.getByTestId("docheader-title").textContent).toBe("db");
  });

  it("hides the abstract / action row / preflight when compact", () => {
    renderWithProviders(
      <DocHeaderShell
        filePath="x.md"
        compact
        frontmatter={{ abstract: "Some text" }}
        onRunAll={() => {}}
        preflightItems={[preflightItem()]}
      />,
    );
    expect(
      screen.getByTestId("docheader-shell").getAttribute("data-compact"),
    ).toBe("true");
    expect(
      screen.queryByTestId("docheader-shell-action-row-slot"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByTestId("docheader-shell-abstract-slot"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByTestId("docheader-shell-preflight-slot"),
    ).not.toBeInTheDocument();
  });

  it("shows the action row + abstract + preflight when not compact", () => {
    renderWithProviders(
      <DocHeaderShell
        filePath="x.md"
        frontmatter={{ abstract: "Some text" }}
        onRunAll={() => {}}
        preflightItems={[preflightItem()]}
      />,
    );
    expect(
      screen.getByTestId("docheader-shell-action-row-slot"),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId("docheader-shell-abstract-slot"),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId("docheader-shell-preflight-slot"),
    ).toBeInTheDocument();
  });

  it("hides the preflight slot when items array is empty even in non-compact mode", () => {
    renderWithProviders(<DocHeaderShell filePath="x.md" preflightItems={[]} />);
    expect(
      screen.queryByTestId("docheader-shell-preflight-slot"),
    ).not.toBeInTheDocument();
  });

  it("forwards onToggleCompact through to the title click (legacy fallback)", async () => {
    const onToggleCompact = vi.fn();
    renderWithProviders(
      <DocHeaderShell filePath="x.md" onToggleCompact={onToggleCompact} />,
    );
    await userEvent.setup().click(screen.getByTestId("docheader-title"));
    expect(onToggleCompact).toHaveBeenCalledTimes(1);
  });

  it("title click prefers onTitleNavigateToBody over onToggleCompact", async () => {
    const onToggleCompact = vi.fn();
    const onTitleNavigateToBody = vi.fn();
    renderWithProviders(
      <DocHeaderShell
        filePath="x.md"
        onToggleCompact={onToggleCompact}
        onTitleNavigateToBody={onTitleNavigateToBody}
      />,
    );
    await userEvent.setup().click(screen.getByTestId("docheader-title"));
    expect(onTitleNavigateToBody).toHaveBeenCalledTimes(1);
    expect(onToggleCompact).not.toHaveBeenCalled();
  });

  it("onTitleNavigateToBody alone wires the title click", async () => {
    const onTitleNavigateToBody = vi.fn();
    renderWithProviders(
      <DocHeaderShell
        filePath="x.md"
        onTitleNavigateToBody={onTitleNavigateToBody}
      />,
    );
    await userEvent.setup().click(screen.getByTestId("docheader-title"));
    expect(onTitleNavigateToBody).toHaveBeenCalledTimes(1);
  });

  it("renders the meta strip when meta props are provided", () => {
    renderWithProviders(
      <DocHeaderShell
        filePath="x.md"
        author={{ name: "Jane Doe", email: null }}
        mtimeMs={Date.now() - 60_000}
        dirty
        branch={{ branch: "main", addedLines: 1, modifiedLines: 0 }}
      />,
    );
    expect(screen.getByTestId("docheader-meta-strip")).toBeInTheDocument();
    expect(screen.getByTestId("docheader-meta-author")).toBeInTheDocument();
    expect(screen.getByTestId("docheader-meta-edited")).toBeInTheDocument();
    expect(screen.getByTestId("docheader-meta-branch")).toBeInTheDocument();
  });

  it("forwards Run-all through the action row", async () => {
    const onRunAll = vi.fn();
    renderWithProviders(<DocHeaderShell filePath="x.md" onRunAll={onRunAll} />);
    await userEvent
      .setup()
      .click(screen.getByTestId("docheader-action-run-all"));
    expect(onRunAll).toHaveBeenCalledTimes(1);
  });

  it("forwards onPreflightRecheck through the pill row", async () => {
    const onPreflightRecheck = vi.fn();
    renderWithProviders(
      <DocHeaderShell
        filePath="x.md"
        preflightItems={[preflightItem()]}
        onPreflightRecheck={onPreflightRecheck}
      />,
    );
    await userEvent
      .setup()
      .click(screen.getByTestId("preflight-pills-recheck"));
    expect(onPreflightRecheck).toHaveBeenCalledTimes(1);
  });

  it("forwards breadcrumb selection through to the card", async () => {
    const onBreadcrumbSelect = vi.fn();
    renderWithProviders(
      <DocHeaderShell
        filePath="notes/db.md"
        relativeFilePath="notes/db.md"
        onBreadcrumbSelect={onBreadcrumbSelect}
      />,
    );
    const seg0 = screen.getByTestId("docheader-breadcrumb-segment-0");
    const button = seg0.querySelector("button");
    await userEvent.setup().click(button!);
    expect(onBreadcrumbSelect).toHaveBeenCalledWith("notes");
  });
});
