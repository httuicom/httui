import { describe, expect, it } from "vitest";

import {
  GitCommitDiffViewer,
  classifyDiffLine,
} from "@/components/layout/git/GitCommitDiffViewer";
import { renderWithProviders, screen } from "@/test/render";

describe("classifyDiffLine", () => {
  it("classifies hunk headers", () => {
    expect(classifyDiffLine("@@ -1,3 +1,4 @@")).toBe("hunk");
  });

  it("classifies file headers", () => {
    expect(classifyDiffLine("diff --git a/x b/x")).toBe("fileheader");
    expect(classifyDiffLine("index 1234..5678 100644")).toBe("fileheader");
    expect(classifyDiffLine("--- a/x")).toBe("fileheader");
    expect(classifyDiffLine("+++ b/x")).toBe("fileheader");
  });

  it("classifies add / remove / context", () => {
    expect(classifyDiffLine("+added")).toBe("add");
    expect(classifyDiffLine("-removed")).toBe("remove");
    expect(classifyDiffLine(" context")).toBe("context");
    expect(classifyDiffLine("")).toBe("context");
  });
});

describe("GitCommitDiffViewer", () => {
  it("shows loading state when diff is null", () => {
    renderWithProviders(<GitCommitDiffViewer diff={null} />);
    expect(
      screen.getByTestId("git-commit-diff-viewer").getAttribute("data-loading"),
    ).toBe("true");
  });

  it("shows empty hint when diff string is empty", () => {
    renderWithProviders(<GitCommitDiffViewer diff="" />);
    expect(
      screen.getByTestId("git-commit-diff-viewer-empty"),
    ).toBeInTheDocument();
  });

  it("renders one row per diff line with role data attribute", () => {
    const diff = [
      "diff --git a/x b/x",
      "index 1..2",
      "--- a/x",
      "+++ b/x",
      "@@ -1,1 +1,2 @@",
      "-old",
      "+new",
      " ctx",
    ].join("\n");
    renderWithProviders(<GitCommitDiffViewer diff={diff} />);
    expect(
      screen.getByTestId("git-commit-diff-line-0").getAttribute("data-role"),
    ).toBe("fileheader");
    expect(
      screen.getByTestId("git-commit-diff-line-4").getAttribute("data-role"),
    ).toBe("hunk");
    expect(
      screen.getByTestId("git-commit-diff-line-5").getAttribute("data-role"),
    ).toBe("remove");
    expect(
      screen.getByTestId("git-commit-diff-line-6").getAttribute("data-role"),
    ).toBe("add");
    expect(
      screen.getByTestId("git-commit-diff-line-7").getAttribute("data-role"),
    ).toBe("context");
  });

  it("renders the header with shortSha + subject when provided", () => {
    renderWithProviders(
      <GitCommitDiffViewer shortSha="abc1234" subject="fix bug" diff="+a" />,
    );
    expect(
      screen.getByTestId("git-commit-diff-viewer-header").textContent,
    ).toMatch(/abc1234.*fix bug/);
  });

  it("truncates at maxLines and surfaces a warning hint", () => {
    const big = Array.from({ length: 10 }, (_, i) => `+${i}`).join("\n");
    renderWithProviders(<GitCommitDiffViewer diff={big} maxLines={3} />);
    expect(
      screen
        .getByTestId("git-commit-diff-viewer")
        .getAttribute("data-truncated"),
    ).toBe("true");
    expect(
      screen.getByTestId("git-commit-diff-viewer-truncation-hint"),
    ).toBeInTheDocument();
    // Lines 0..2 rendered, line 3 not.
    expect(screen.getByTestId("git-commit-diff-line-0")).toBeInTheDocument();
    expect(
      screen.queryByTestId("git-commit-diff-line-3"),
    ).not.toBeInTheDocument();
  });

  it("encodes total line count in data-line-count", () => {
    renderWithProviders(<GitCommitDiffViewer diff={"a\nb\nc"} />);
    expect(
      screen
        .getByTestId("git-commit-diff-viewer")
        .getAttribute("data-line-count"),
    ).toBe("3");
  });
});
