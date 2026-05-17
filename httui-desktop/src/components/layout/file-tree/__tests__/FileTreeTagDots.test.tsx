import { afterEach, beforeEach, describe, expect, it } from "vitest";

import {
  FileTreeTagDots,
  pickColor,
} from "@/components/layout/file-tree/FileTreeTagDots";
import { useTagIndexStore } from "@/stores/tagIndex";
import { renderWithProviders, screen } from "@/test/render";

describe("FileTreeTagDots", () => {
  beforeEach(() => {
    useTagIndexStore.getState().clearAll();
  });

  afterEach(() => {
    useTagIndexStore.getState().clearAll();
  });

  it("renders nothing when the file has no tags", () => {
    renderWithProviders(<FileTreeTagDots filePath="/v/x.md" />);
    expect(screen.queryByTestId("file-tree-tag-dots")).not.toBeInTheDocument();
  });

  it("renders one dot per tag for files with up to 3 tags", () => {
    useTagIndexStore.getState().setTagsForFile("/v/x.md", ["api", "payments"]);
    renderWithProviders(<FileTreeTagDots filePath="/v/x.md" />);
    expect(screen.getByTestId("file-tree-tag-dots")).toBeInTheDocument();
    expect(screen.getByTestId("file-tree-tag-dot-api")).toBeInTheDocument();
    expect(
      screen.getByTestId("file-tree-tag-dot-payments"),
    ).toBeInTheDocument();
    expect(
      screen.queryByTestId("file-tree-tag-dots-overflow"),
    ).not.toBeInTheDocument();
  });

  it("caps the visible dots at 3 and shows an overflow counter", () => {
    useTagIndexStore
      .getState()
      .setTagsForFile("/v/y.md", ["a", "b", "c", "d", "e"]);
    renderWithProviders(<FileTreeTagDots filePath="/v/y.md" />);
    expect(screen.getByTestId("file-tree-tag-dot-a")).toBeInTheDocument();
    expect(screen.getByTestId("file-tree-tag-dot-b")).toBeInTheDocument();
    expect(screen.getByTestId("file-tree-tag-dot-c")).toBeInTheDocument();
    expect(screen.queryByTestId("file-tree-tag-dot-d")).not.toBeInTheDocument();
    expect(screen.getByTestId("file-tree-tag-dots-overflow").textContent).toBe(
      "+2",
    );
  });

  it("the wrapper title attribute lists every tag (incl. overflow)", () => {
    useTagIndexStore.getState().setTagsForFile("/v/z.md", ["a", "b", "c", "d"]);
    renderWithProviders(<FileTreeTagDots filePath="/v/z.md" />);
    expect(screen.getByTestId("file-tree-tag-dots").title).toBe("a, b, c, d");
  });

  it("re-renders when the store updates the file's tags", () => {
    const { rerender } = renderWithProviders(
      <FileTreeTagDots filePath="/v/r.md" />,
    );
    expect(screen.queryByTestId("file-tree-tag-dots")).not.toBeInTheDocument();

    useTagIndexStore.getState().setTagsForFile("/v/r.md", ["api"]);
    rerender(<FileTreeTagDots filePath="/v/r.md" />);
    expect(screen.getByTestId("file-tree-tag-dot-api")).toBeInTheDocument();
  });
});

describe("pickColor", () => {
  it("is stable across calls for the same tag", () => {
    const a = pickColor("payments");
    const b = pickColor("payments");
    expect(a).toBe(b);
  });

  it("returns one of the palette tokens", () => {
    const palette = new Set([
      "blue.solid",
      "purple.solid",
      "teal.solid",
      "orange.solid",
      "pink.solid",
      "green.solid",
    ]);
    for (const t of ["api", "payments", "auth", "x", "x".repeat(40)]) {
      expect(palette.has(pickColor(t))).toBe(true);
    }
  });

  it("never throws on the empty string", () => {
    expect(() => pickColor("")).not.toThrow();
  });
});
