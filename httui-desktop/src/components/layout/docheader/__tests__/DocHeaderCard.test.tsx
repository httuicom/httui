import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { DocHeaderCard } from "@/components/layout/docheader/DocHeaderCard";
import { renderWithProviders, screen } from "@/test/render";

describe("DocHeaderCard", () => {
  it("renders the H1 from frontmatter title when present", () => {
    renderWithProviders(
      <DocHeaderCard
        filePath="notes/db.md"
        frontmatter={{ title: "DB Runbook" }}
      />,
    );
    expect(screen.getByTestId("docheader-title").textContent).toBe(
      "DB Runbook",
    );
  });

  it("falls back to firstHeading when frontmatter has no title", () => {
    renderWithProviders(
      <DocHeaderCard filePath="notes/db.md" firstHeading="Database Runbook" />,
    );
    expect(screen.getByTestId("docheader-title").textContent).toBe(
      "Database Runbook",
    );
  });

  it("falls back to the filename when nothing else is available", () => {
    renderWithProviders(<DocHeaderCard filePath="notes/db.md" />);
    expect(screen.getByTestId("docheader-title").textContent).toBe("db");
  });

  it("hides the breadcrumb when no relativeFilePath is provided", () => {
    renderWithProviders(<DocHeaderCard filePath="notes/db.md" />);
    expect(
      screen.queryByTestId("docheader-breadcrumb"),
    ).not.toBeInTheDocument();
  });

  it("hides the breadcrumb for a flat file path", () => {
    renderWithProviders(
      <DocHeaderCard filePath="scratch.md" relativeFilePath="scratch.md" />,
    );
    expect(
      screen.queryByTestId("docheader-breadcrumb"),
    ).not.toBeInTheDocument();
  });

  it("renders one breadcrumb segment per path component", () => {
    renderWithProviders(
      <DocHeaderCard
        filePath="notes/runbooks/db.md"
        relativeFilePath="notes/runbooks/db.md"
      />,
    );
    expect(
      screen.getByTestId("docheader-breadcrumb-segment-0"),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId("docheader-breadcrumb-segment-1"),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId("docheader-breadcrumb-segment-2"),
    ).toBeInTheDocument();
  });

  it("renders breadcrumb non-leaf segments as buttons when onBreadcrumbSelect is provided", async () => {
    const onBreadcrumbSelect = vi.fn();
    renderWithProviders(
      <DocHeaderCard
        filePath="notes/runbooks/db.md"
        relativeFilePath="notes/runbooks/db.md"
        onBreadcrumbSelect={onBreadcrumbSelect}
      />,
    );
    const seg0 = screen.getByTestId("docheader-breadcrumb-segment-0");
    const button = seg0.querySelector("button");
    expect(button).toBeTruthy();
    await userEvent.setup().click(button!);
    expect(onBreadcrumbSelect).toHaveBeenCalledTimes(1);
    expect(onBreadcrumbSelect.mock.calls[0]![0]).toBe("notes");
  });

  it("renders the leaf segment as inactive even with onBreadcrumbSelect", () => {
    renderWithProviders(
      <DocHeaderCard
        filePath="notes/db.md"
        relativeFilePath="notes/db.md"
        onBreadcrumbSelect={() => {}}
      />,
    );
    const leaf = screen.getByTestId("docheader-breadcrumb-segment-1");
    expect(leaf.querySelector("button")).toBeNull();
    expect(leaf.querySelector("[data-leaf='true']")).toBeInTheDocument();
  });

  it("flags compact mode via data-compact", () => {
    renderWithProviders(<DocHeaderCard filePath="x.md" compact />);
    expect(
      screen.getByTestId("docheader-card").getAttribute("data-compact"),
    ).toBe("true");
  });

  it("renders the title as a button when onTitleClick is provided", async () => {
    const onTitleClick = vi.fn();
    renderWithProviders(
      <DocHeaderCard filePath="x.md" onTitleClick={onTitleClick} />,
    );
    const title = screen.getByTestId("docheader-title");
    expect(title.tagName).toBe("BUTTON");
    await userEvent.setup().click(title);
    expect(onTitleClick).toHaveBeenCalledTimes(1);
  });

  it("renders the title as h1 by default", () => {
    renderWithProviders(<DocHeaderCard filePath="x.md" />);
    expect(screen.getByTestId("docheader-title").tagName).toBe("H1");
  });

  describe("editable mode (onTitleSave)", () => {
    beforeEach(() => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
    });
    afterEach(() => {
      vi.useRealTimers();
    });

    it("renders an input when onTitleSave is provided", () => {
      renderWithProviders(
        <DocHeaderCard
          filePath="x.md"
          frontmatter={{ title: "Initial" }}
          onTitleSave={() => {}}
        />,
      );
      const input = screen.getByTestId("docheader-title");
      expect(input.tagName).toBe("INPUT");
      expect((input as HTMLInputElement).value).toBe("Initial");
    });

    it("renders an empty input with `Untitled` placeholder in virtual mode", () => {
      renderWithProviders(
        <DocHeaderCard filePath="x.md" onTitleSave={() => {}} />,
      );
      const input = screen.getByTestId("docheader-title") as HTMLInputElement;
      expect(input.value).toBe("");
      expect(input.placeholder).toBe("Untitled");
    });

    it("does not fall back to the filename in editable mode", () => {
      renderWithProviders(
        <DocHeaderCard filePath="my-note.md" onTitleSave={() => {}} />,
      );
      const input = screen.getByTestId("docheader-title") as HTMLInputElement;
      // Static path would have shown "my-note"; editable mode uses
      // empty + placeholder so the user explicitly types a title before
      // the disk write happens.
      expect(input.value).toBe("");
    });

    it("debounces onTitleSave by 300ms after the last keystroke", async () => {
      const onTitleSave = vi.fn();
      const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
      renderWithProviders(
        <DocHeaderCard
          filePath="x.md"
          frontmatter={{ title: "" }}
          onTitleSave={onTitleSave}
        />,
      );
      const input = screen.getByTestId("docheader-title") as HTMLInputElement;
      await user.click(input);
      await user.keyboard("Hello");
      expect(onTitleSave).not.toHaveBeenCalled();
      vi.advanceTimersByTime(300);
      expect(onTitleSave).toHaveBeenCalledTimes(1);
      expect(onTitleSave).toHaveBeenCalledWith("Hello");
    });

    it("re-syncs the input when the external frontmatter title changes", () => {
      const { rerender } = renderWithProviders(
        <DocHeaderCard
          filePath="x.md"
          frontmatter={{ title: "First" }}
          onTitleSave={() => {}}
        />,
      );
      expect(
        (screen.getByTestId("docheader-title") as HTMLInputElement).value,
      ).toBe("First");

      rerender(
        <DocHeaderCard
          filePath="x.md"
          frontmatter={{ title: "Second" }}
          onTitleSave={() => {}}
        />,
      );
      expect(
        (screen.getByTestId("docheader-title") as HTMLInputElement).value,
      ).toBe("Second");
    });
  });

  describe("frontmatter error badge", () => {
    it("shows the error badge when frontmatter.error is set", () => {
      renderWithProviders(
        <DocHeaderCard
          filePath="notes/db.md"
          frontmatter={{
            title: "x",
            error: "frontmatter inválido: bloco não fechado (faltando `---`)",
          }}
        />,
      );
      const badge = screen.getByTestId("docheader-frontmatter-error");
      expect(badge.textContent).toMatch(/não fechado/);
    });

    it("hides the badge when frontmatter has no error", () => {
      renderWithProviders(
        <DocHeaderCard filePath="notes/db.md" frontmatter={{ title: "x" }} />,
      );
      expect(
        screen.queryByTestId("docheader-frontmatter-error"),
      ).not.toBeInTheDocument();
    });

    it("hides the badge when no frontmatter is provided", () => {
      renderWithProviders(<DocHeaderCard filePath="notes/db.md" />);
      expect(
        screen.queryByTestId("docheader-frontmatter-error"),
      ).not.toBeInTheDocument();
    });
  });
});
