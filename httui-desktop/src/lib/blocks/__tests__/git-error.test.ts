import { describe, expect, it } from "vitest";

import { formatGitError } from "@/lib/blocks/git-error";

const GH013 =
  "remote: error: GH013: Repository rule violations found for refs/heads/main. " +
  "remote: Review all repository rules at https://github.com/gandarfh/secret-test/rules?ref=refs%2Fheads%2Fmain " +
  "remote: - Changes must be made through a pull request. " +
  "remote: To github.com:gandarfh/secret-test.git " +
  "! [remote rejected] main -> main (push declined due to repository rule violations) " +
  "error: failed to push some refs to 'github.com:gandarfh/secret-test.git'";

describe("formatGitError", () => {
  it("summarizes a branch-rule rejection as 'requires a pull request'", () => {
    const { summary } = formatGitError(GH013);
    expect(summary).toBe(
      "Push rejected — this branch requires a pull request.",
    );
  });

  it("strips the remote: noise and breaks the run-on into lines", () => {
    const { detail } = formatGitError(GH013);
    expect(detail).not.toMatch(/remote:/);
    const lines = detail.split("\n");
    expect(lines.length).toBeGreaterThan(3);
    expect(detail).toContain(
      "GH013: Repository rule violations found for refs/heads/main.",
    );
    expect(lines.some((l) => l.startsWith("! [remote rejected]"))).toBe(true);
    expect(lines.some((l) => l.startsWith("error: failed to push"))).toBe(true);
  });

  it("detects a non-fast-forward rejection", () => {
    const raw =
      "! [rejected] main -> main (fetch first) " +
      "error: failed to push some refs hint: Updates were rejected because " +
      "the tip of your current branch is behind";
    expect(formatGitError(raw).summary).toBe(
      "Push rejected — your branch is behind. Pull first.",
    );
  });

  it("detects an auth failure", () => {
    expect(
      formatGitError("fatal: Authentication failed for 'https://x/y.git'")
        .summary,
    ).toBe("Authentication failed — check your git credentials or SSH key.");
  });

  it("falls back to the first informative line for unknown errors", () => {
    const { summary } = formatGitError(
      "fatal: something unusual happened\nTo github.com:o/r.git",
    );
    expect(summary).toBe("fatal: something unusual happened");
  });

  it("never returns an empty summary", () => {
    expect(formatGitError("   ").summary).toBe("Git command failed.");
  });
});
