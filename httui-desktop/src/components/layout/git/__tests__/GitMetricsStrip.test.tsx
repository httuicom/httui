import { describe, expect, it } from "vitest";

import { renderWithProviders, screen } from "@/test/render";
import { GitMetricsStrip } from "@/components/layout/git/GitMetricsStrip";
import type { CommitInfo, GitStatus } from "@/lib/tauri/git";

const status: GitStatus = {
  branch: "feature/x",
  upstream: "origin/feature/x",
  ahead: 2,
  behind: 1,
  clean: false,
  changed: [
    { path: "a.md", status: " M", staged: false, untracked: false },
    { path: "b.md", status: "A.", staged: true, untracked: false },
    { path: "c.md", status: "??", staged: false, untracked: true },
  ],
};

const head: CommitInfo = {
  sha: "abc",
  short_sha: "abc1234",
  author_name: "Ada",
  author_email: "ada@x.dev",
  timestamp: Math.floor(Date.now() / 1000) - 120,
  subject: "do a thing",
};

describe("GitMetricsStrip", () => {
  it("renders branch, upstream and explicit ahead/behind", () => {
    renderWithProviders(
      <GitMetricsStrip
        status={status}
        commits={[head]}
        remotes={[{ name: "origin", url: "git@github.com:o/r.git" }]}
        lastSyncAt={null}
      />,
    );
    expect(screen.getByTestId("git-metric-branch")).toHaveTextContent(
      "feature/x",
    );
    expect(screen.getByTestId("git-metric-upstream")).toHaveTextContent(
      "origin/feature/x",
    );
    expect(screen.getByTestId("git-metric-aheadbehind")).toHaveTextContent(
      "↑2 ↓1",
    );
  });

  it("counts changes by type", () => {
    renderWithProviders(
      <GitMetricsStrip
        status={status}
        commits={[head]}
        remotes={[]}
        lastSyncAt={null}
      />,
    );
    expect(screen.getByTestId("git-metric-changes")).toHaveTextContent(
      "M1 A1 D0 ?1 U0",
    );
  });

  it("shows last commit author and the remote URL", () => {
    renderWithProviders(
      <GitMetricsStrip
        status={status}
        commits={[head]}
        remotes={[{ name: "origin", url: "git@github.com:o/r.git" }]}
        lastSyncAt={null}
      />,
    );
    expect(screen.getByTestId("git-metric-lastcommit")).toHaveTextContent(
      "Ada",
    );
    expect(screen.getByTestId("git-metric-remote")).toHaveTextContent(
      "git@github.com:o/r.git",
    );
  });

  it("renders 'never' until a sync stamps, then a relative time", () => {
    const { rerender } = renderWithProviders(
      <GitMetricsStrip
        status={status}
        commits={[]}
        remotes={[]}
        lastSyncAt={null}
      />,
    );
    expect(screen.getByTestId("git-metric-lastsync")).toHaveTextContent(
      "never",
    );
    expect(screen.getByTestId("git-metric-lastcommit")).toHaveTextContent("—");
    rerender(
      <GitMetricsStrip
        status={status}
        commits={[]}
        remotes={[]}
        lastSyncAt={Date.now() - 5000}
      />,
    );
    expect(screen.getByTestId("git-metric-lastsync")).not.toHaveTextContent(
      "never",
    );
  });
});
