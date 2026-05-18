// the side panel and the pane-tab must never
// diverge: they read/write the same useGitStore. This renders BOTH
// at once over one in-memory git backend and proves a stage / commit
// done in the side panel surfaces in the pane-tab.

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import userEvent from "@testing-library/user-event";

import { renderWithProviders, screen, waitFor, within } from "@/test/render";
import { clearTauriMocks, mockTauriCommand } from "@/test/mocks/tauri";
import { useWorkspaceStore } from "@/stores/workspace";
import { GitSidePanel } from "@/components/layout/git/GitSidePanel";
import { GitPanelContainer } from "@/components/layout/git/GitPanelContainer";

interface Backend {
  staged: boolean;
  committed: string | null;
}

function wireBackend(b: Backend) {
  mockTauriCommand("git_status_cmd", () => ({
    branch: "main",
    upstream: "origin/main",
    ahead: b.committed ? 1 : 0,
    behind: 0,
    clean: b.committed !== null,
    changed: b.committed
      ? []
      : [
          {
            path: "foo.md",
            status: b.staged ? "M." : " M",
            staged: b.staged,
            untracked: false,
          },
        ],
  }));
  mockTauriCommand("git_remote_list_cmd", () => []);
  mockTauriCommand("stage_path_cmd", () => {
    b.staged = true;
  });
  mockTauriCommand("unstage_path_cmd", () => {
    b.staged = false;
  });
  mockTauriCommand("git_commit_cmd", (a) => {
    b.committed = (a as { message: string }).message;
  });
  mockTauriCommand("git_log_cmd", () =>
    b.committed
      ? [
          {
            sha: "c1",
            short_sha: "c1",
            author_name: "Ada",
            author_email: "a@x.dev",
            timestamp: 1_700_000_000,
            subject: b.committed,
          },
        ]
      : [],
  );
}

beforeEach(() => {
  clearTauriMocks();
  useWorkspaceStore.setState({ vaultPath: "/v" });
});
afterEach(() => {
  clearTauriMocks();
  useWorkspaceStore.setState({ vaultPath: null });
});

describe("git side panel ↔ pane-tab consistency", () => {
  it("a stage in the side panel surfaces as staged in the pane-tab", async () => {
    wireBackend({ staged: false, committed: null });
    const user = userEvent.setup();
    renderWithProviders(
      <>
        <GitSidePanel width={340} onClose={() => {}} />
        <GitPanelContainer />
      </>,
    );

    // Re-query the live nodes on every assertion — React replaces
    // subtrees on re-render, so a cached `within(node)` would go
    // stale and the query would miss.
    const inPane = () => within(screen.getByTestId("git-panel"));
    const inSide = () => within(screen.getByTestId("git-side-panel"));

    await screen.findByTestId("git-panel");
    await waitFor(() =>
      expect(inPane().getByTestId("git-file-row-foo.md")).not.toHaveAttribute(
        "data-staged",
      ),
    );

    await user.click(inSide().getByTestId("git-file-row-foo.md-stage"));

    // The pane-tab (same store) now shows it staged — no divergence.
    await waitFor(() =>
      expect(inPane().getByTestId("git-file-row-foo.md")).toHaveAttribute(
        "data-staged",
        "true",
      ),
    );
  });

  it("a commit in the side panel clears the pane-tab working tree", async () => {
    const backend: Backend = { staged: true, committed: null };
    wireBackend(backend);
    const user = userEvent.setup();
    renderWithProviders(
      <>
        <GitSidePanel width={340} onClose={() => {}} />
        <GitPanelContainer />
      </>,
    );

    const inSide = () => within(screen.getByTestId("git-side-panel"));

    await screen.findByTestId("git-side-panel");
    await screen.findByTestId("git-panel");

    // foo.md is staged → side panel commit button is enabled.
    await waitFor(() =>
      expect(inSide().getByTestId("git-commit-form-submit")).toBeEnabled(),
    );
    await user.click(inSide().getByTestId("git-commit-form-submit"));

    // Both surfaces reflect the now-clean tree (same store).
    await waitFor(() =>
      expect(screen.getByTestId("git-panel")).toHaveAttribute(
        "data-clean",
        "true",
      ),
    );
  });
});
