import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { renderWithProviders, screen, waitFor } from "@/test/render";
import { clearTauriMocks, mockTauriCommand } from "@/test/mocks/tauri";
import { GitSidePanelHistory } from "@/components/layout/git/GitSidePanelHistory";
import type { CommitInfo } from "@/lib/tauri/git";

const commit = (n: number): CommitInfo => ({
  sha: `sha${n}`,
  short_sha: `s${n}`,
  author_name: "Ada",
  author_email: "ada@x.dev",
  timestamp: 1_700_000_000 + n,
  subject: `commit ${n}`,
});

beforeEach(() => clearTauriMocks());
afterEach(() => clearTauriMocks());

describe("GitSidePanelHistory", () => {
  it("renders the recent commits", () => {
    renderWithProviders(
      <GitSidePanelHistory
        vaultPath="/v"
        commits={[commit(1), commit(2)]}
        onViewAll={() => {}}
      />,
    );
    expect(screen.getByTestId("git-log-row-s1")).toBeInTheDocument();
    expect(screen.getByTestId("git-log-row-s2")).toBeInTheDocument();
  });

  it("caps the list at the limit", () => {
    const many = Array.from({ length: 15 }, (_, i) => commit(i));
    renderWithProviders(
      <GitSidePanelHistory
        vaultPath="/v"
        commits={many}
        onViewAll={() => {}}
        limit={5}
      />,
    );
    expect(screen.getByTestId("git-log-list")).toHaveAttribute(
      "data-count",
      "5",
    );
  });

  it("opens the inline diff when a commit is clicked", async () => {
    mockTauriCommand("git_diff_cmd", () => "diff --git a b\n+added line");
    const user = userEvent.setup();
    renderWithProviders(
      <GitSidePanelHistory
        vaultPath="/v"
        commits={[commit(1)]}
        onViewAll={() => {}}
      />,
    );
    await user.click(screen.getByTestId("git-log-row-s1"));
    const viewer = await screen.findByTestId("git-commit-diff-viewer");
    await waitFor(() => expect(viewer).toHaveAttribute("data-line-count", "2"));
  });

  it("toggles the diff off when the same commit is re-clicked", async () => {
    mockTauriCommand("git_diff_cmd", () => "x");
    const user = userEvent.setup();
    renderWithProviders(
      <GitSidePanelHistory
        vaultPath="/v"
        commits={[commit(1)]}
        onViewAll={() => {}}
      />,
    );
    await user.click(screen.getByTestId("git-log-row-s1"));
    await screen.findByTestId("git-commit-diff-viewer");
    await user.click(screen.getByTestId("git-log-row-s1"));
    await waitFor(() =>
      expect(screen.queryByTestId("git-commit-diff-viewer")).toBeNull(),
    );
  });

  it("View all routes to the pane-tab", async () => {
    const onViewAll = vi.fn();
    const user = userEvent.setup();
    renderWithProviders(
      <GitSidePanelHistory
        vaultPath="/v"
        commits={[commit(1)]}
        onViewAll={onViewAll}
      />,
    );
    await user.click(screen.getByTestId("git-side-panel-history-view-all"));
    expect(onViewAll).toHaveBeenCalledTimes(1);
  });
});
