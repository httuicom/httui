import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { DocHeaderAbstract } from "@/components/layout/docheader/DocHeaderAbstract";
import { ABSTRACT_FADE_THRESHOLD } from "@/components/layout/docheader/docheader-derive";
import { renderWithProviders, screen } from "@/test/render";

describe("DocHeaderAbstract", () => {
  it("renders nothing when frontmatter is null", () => {
    renderWithProviders(<DocHeaderAbstract frontmatter={null} />);
    expect(
      screen.queryByTestId("docheader-abstract"),
    ).not.toBeInTheDocument();
  });

  it("renders nothing when frontmatter has no abstract", () => {
    renderWithProviders(<DocHeaderAbstract frontmatter={{}} />);
    expect(
      screen.queryByTestId("docheader-abstract"),
    ).not.toBeInTheDocument();
  });

  it("renders nothing when abstract is whitespace-only", () => {
    renderWithProviders(
      <DocHeaderAbstract frontmatter={{ abstract: "   " }} />,
    );
    expect(
      screen.queryByTestId("docheader-abstract"),
    ).not.toBeInTheDocument();
  });

  it("renders a short abstract without truncation hints", () => {
    renderWithProviders(
      <DocHeaderAbstract frontmatter={{ abstract: "Short summary." }} />,
    );
    expect(screen.getByTestId("docheader-abstract-text").textContent).toBe(
      "Short summary.",
    );
    expect(
      screen.queryByTestId("docheader-abstract-toggle"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByTestId("docheader-abstract-fade"),
    ).not.toBeInTheDocument();
    expect(
      screen
        .getByTestId("docheader-abstract")
        .getAttribute("data-clamped"),
    ).toBeNull();
  });

  it("renders the toggle + fade for long abstracts (>250 chars)", () => {
    const long = "x".repeat(ABSTRACT_FADE_THRESHOLD + 1);
    renderWithProviders(
      <DocHeaderAbstract frontmatter={{ abstract: long }} />,
    );
    expect(screen.getByTestId("docheader-abstract-toggle")).toBeInTheDocument();
    expect(screen.getByTestId("docheader-abstract-fade")).toBeInTheDocument();
    expect(
      screen
        .getByTestId("docheader-abstract")
        .getAttribute("data-clamped"),
    ).toBe("true");
  });

  it("toggle reads 'more' when collapsed and 'less' when expanded", async () => {
    const long = "x".repeat(ABSTRACT_FADE_THRESHOLD + 1);
    renderWithProviders(
      <DocHeaderAbstract frontmatter={{ abstract: long }} />,
    );
    const toggle = screen.getByTestId("docheader-abstract-toggle");
    expect(toggle.textContent).toBe("more");
    await userEvent.setup().click(toggle);
    expect(toggle.textContent).toBe("less");
  });

  it("removes the fade and clamp flag when expanded", async () => {
    const long = "x".repeat(ABSTRACT_FADE_THRESHOLD + 1);
    renderWithProviders(
      <DocHeaderAbstract frontmatter={{ abstract: long }} />,
    );
    await userEvent
      .setup()
      .click(screen.getByTestId("docheader-abstract-toggle"));
    expect(
      screen.queryByTestId("docheader-abstract-fade"),
    ).not.toBeInTheDocument();
    expect(
      screen
        .getByTestId("docheader-abstract")
        .getAttribute("data-clamped"),
    ).toBeNull();
  });

  it("trims surrounding whitespace before rendering", () => {
    renderWithProviders(
      <DocHeaderAbstract frontmatter={{ abstract: "  Hello  " }} />,
    );
    expect(screen.getByTestId("docheader-abstract-text").textContent).toBe(
      "Hello",
    );
  });

  describe("editable mode (onAbstractSave)", () => {
    beforeEach(() => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
    });
    afterEach(() => {
      vi.useRealTimers();
    });

    it("renders a textarea instead of static text when onAbstractSave is given", () => {
      renderWithProviders(
        <DocHeaderAbstract
          frontmatter={{ abstract: "An abstract" }}
          onAbstractSave={() => {}}
        />,
      );
      const input = screen.getByTestId(
        "docheader-abstract-input",
      ) as HTMLTextAreaElement;
      expect(input.tagName).toBe("TEXTAREA");
      expect(input.value).toBe("An abstract");
      // Static text node is gone — the editable mode owns the slot.
      expect(
        screen.queryByTestId("docheader-abstract-text"),
      ).not.toBeInTheDocument();
    });

    it("renders an empty textarea with placeholder when no abstract is set", () => {
      renderWithProviders(
        <DocHeaderAbstract frontmatter={null} onAbstractSave={() => {}} />,
      );
      const input = screen.getByTestId(
        "docheader-abstract-input",
      ) as HTMLTextAreaElement;
      expect(input.value).toBe("");
      expect(input.placeholder).toBe("Add a description…");
    });

    it("debounces onAbstractSave by 400ms after the last keystroke", async () => {
      const onAbstractSave = vi.fn();
      const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
      renderWithProviders(
        <DocHeaderAbstract
          frontmatter={{ abstract: "" }}
          onAbstractSave={onAbstractSave}
        />,
      );
      const input = screen.getByTestId(
        "docheader-abstract-input",
      ) as HTMLTextAreaElement;
      await user.click(input);
      await user.keyboard("Notes");
      expect(onAbstractSave).not.toHaveBeenCalled();
      vi.advanceTimersByTime(400);
      expect(onAbstractSave).toHaveBeenCalledTimes(1);
      expect(onAbstractSave).toHaveBeenCalledWith("Notes");
    });

    it("commits on Enter without inserting a newline", async () => {
      const onAbstractSave = vi.fn();
      const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
      renderWithProviders(
        <DocHeaderAbstract
          frontmatter={{ abstract: "" }}
          onAbstractSave={onAbstractSave}
        />,
      );
      const input = screen.getByTestId(
        "docheader-abstract-input",
      ) as HTMLTextAreaElement;
      await user.click(input);
      await user.keyboard("Hi{Enter}");
      // Newline never makes it into the value.
      expect(input.value).toBe("Hi");
      vi.advanceTimersByTime(400);
      expect(onAbstractSave).toHaveBeenCalledWith("Hi");
    });
  });
});
