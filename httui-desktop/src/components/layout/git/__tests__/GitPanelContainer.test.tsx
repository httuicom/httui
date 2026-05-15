import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { renderWithProviders, screen, waitFor } from "@/test/render";
import userEvent from "@testing-library/user-event";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";

import { GitPanelContainer } from "@/components/layout/git/GitPanelContainer";
import { useWorkspaceStore } from "@/stores/workspace";
import { usePaneStore } from "@/stores/pane";
import type { CommitInfo, GitStatus } from "@/lib/tauri/git";

const cleanStatus: GitStatus = {
  branch: "main",
  upstream: "origin/main",
  ahead: 0,
  behind: 0,
  changed: [],
  clean: true,
};

const dirtyStatus: GitStatus = {
  branch: "main",
  upstream: "origin/main",
  ahead: 0,
  behind: 0,
  changed: [
    { path: "a.md", status: ".M", staged: false, untracked: false },
    { path: "b.md", status: "M.", staged: true, untracked: false },
  ],
  clean: false,
};

const oneCommit: CommitInfo[] = [
  {
    sha: "deadbeef0000000000000000000000000000aaaa",
    short_sha: "deadbee",
    author_name: "Jane Doe",
    author_email: "jane@x.test",
    timestamp: Math.floor(Date.now() / 1000) - 30,
    subject: "first commit",
  },
];

let logCalls = 0;

beforeEach(() => {
  logCalls = 0;
  useWorkspaceStore.setState({ vaultPath: "/tmp/vault" });
  usePaneStore.setState({ saveSignal: 0 });
  clearTauriMocks();
  mockTauriCommand("git_status_cmd", () => cleanStatus);
  mockTauriCommand("git_log_cmd", () => {
    logCalls += 1;
    return oneCommit;
  });
  mockTauriCommand("stage_path_cmd", () => undefined);
  mockTauriCommand("unstage_path_cmd", () => undefined);
  mockTauriCommand("git_commit_cmd", () => undefined);
  mockTauriCommand("git_diff_cmd", () => "diff --git a/a.md b/a.md\n+x");
});

afterEach(() => {
  clearTauriMocks();
  useWorkspaceStore.setState({ vaultPath: null });
  usePaneStore.setState({ saveSignal: 0 });
});

describe("GitPanelContainer", () => {
  it("renders the panel with tabs once status resolves", async () => {
    renderWithProviders(<GitPanelContainer />);
    await waitFor(() => {
      expect(screen.getByTestId("git-panel-tabs")).toBeInTheDocument();
    });
    expect(screen.getByTestId("git-panel").getAttribute("data-clean")).toBe(
      "true",
    );
  });

  it("loads the git log and shows it under the Log tab", async () => {
    const user = userEvent.setup();
    renderWithProviders(<GitPanelContainer />);
    await waitFor(() => {
      expect(screen.getByTestId("git-panel-tabs")).toBeInTheDocument();
    });
    await user.click(screen.getByTestId("git-tab-log"));
    expect(screen.getByTestId("git-log-row-deadbee")).toBeInTheDocument();
    expect(logCalls).toBeGreaterThanOrEqual(1);
  });

  it("re-fetches the log when a save lands (saveSignal bump)", async () => {
    renderWithProviders(<GitPanelContainer />);
    await waitFor(() => {
      expect(screen.getByTestId("git-panel-tabs")).toBeInTheDocument();
    });
    const before = logCalls;
    usePaneStore.setState({ saveSignal: 1 });
    await waitFor(() => {
      expect(logCalls).toBeGreaterThan(before);
    });
  });

  it("shows the loading state when no vault is open", () => {
    useWorkspaceStore.setState({ vaultPath: null });
    renderWithProviders(<GitPanelContainer />);
    expect(
      screen.getByTestId("git-panel").getAttribute("data-loading"),
    ).toBe("true");
  });

  describe("stage + commit (cenário 2)", () => {
    it("stages an unstaged file via stage_path_cmd", async () => {
      let staged = 0;
      mockTauriCommand("git_status_cmd", () => dirtyStatus);
      mockTauriCommand("stage_path_cmd", () => {
        staged += 1;
        return undefined;
      });
      const user = userEvent.setup();
      renderWithProviders(<GitPanelContainer />);
      await waitFor(() => {
        expect(
          screen.getByTestId("git-file-row-a.md-stage"),
        ).toBeInTheDocument();
      });
      await user.click(screen.getByTestId("git-file-row-a.md-stage"));
      await waitFor(() => expect(staged).toBe(1));
    });

    it("unstages a staged file via unstage_path_cmd", async () => {
      let unstaged = 0;
      mockTauriCommand("git_status_cmd", () => dirtyStatus);
      mockTauriCommand("unstage_path_cmd", () => {
        unstaged += 1;
        return undefined;
      });
      const user = userEvent.setup();
      renderWithProviders(<GitPanelContainer />);
      await waitFor(() => {
        expect(
          screen.getByTestId("git-file-row-b.md-stage"),
        ).toBeInTheDocument();
      });
      await user.click(screen.getByTestId("git-file-row-b.md-stage"));
      await waitFor(() => expect(unstaged).toBe(1));
    });

    it("commits and clears the message", async () => {
      let committed = "";
      mockTauriCommand("git_status_cmd", () => dirtyStatus);
      mockTauriCommand("git_commit_cmd", (args) => {
        committed = (args as { message: string }).message;
        return undefined;
      });
      const user = userEvent.setup();
      renderWithProviders(<GitPanelContainer />);
      await waitFor(() => {
        expect(screen.getByTestId("git-commit-form")).toBeInTheDocument();
      });
      const box = screen.getByTestId("git-commit-form-message");
      await user.type(box, "fix: typo");
      await user.click(screen.getByTestId("git-commit-form-submit"));
      await waitFor(() => expect(committed).toBe("fix: typo"));
      await waitFor(() =>
        expect(
          (
            screen.getByTestId(
              "git-commit-form-message",
            ) as HTMLTextAreaElement
          ).value,
        ).toBe(""),
      );
    });

    it("fetches the working diff when a file is selected", async () => {
      let diffCalls = 0;
      mockTauriCommand("git_status_cmd", () => dirtyStatus);
      mockTauriCommand("git_diff_cmd", () => {
        diffCalls += 1;
        return "diff --git a/a.md b/a.md\n+added line";
      });
      const user = userEvent.setup();
      renderWithProviders(<GitPanelContainer />);
      await waitFor(() => {
        expect(screen.getByTestId("git-file-row-a.md")).toBeInTheDocument();
      });
      await user.click(screen.getByTestId("git-file-row-a.md"));
      await waitFor(() => {
        expect(
          screen.getByTestId("git-panel-section-diff"),
        ).toBeInTheDocument();
      });
      expect(diffCalls).toBeGreaterThanOrEqual(1);
      expect(screen.getByText("+added line")).toBeInTheDocument();
    });

    it("hides the diff when the same file is clicked again", async () => {
      mockTauriCommand("git_status_cmd", () => dirtyStatus);
      const user = userEvent.setup();
      renderWithProviders(<GitPanelContainer />);
      await waitFor(() => {
        expect(screen.getByTestId("git-file-row-a.md")).toBeInTheDocument();
      });
      await user.click(screen.getByTestId("git-file-row-a.md"));
      await waitFor(() => {
        expect(
          screen.getByTestId("git-panel-section-diff"),
        ).toBeInTheDocument();
      });
      await user.click(screen.getByTestId("git-file-row-a.md"));
      await waitFor(() => {
        expect(
          screen.queryByTestId("git-panel-section-diff"),
        ).not.toBeInTheDocument();
      });
    });

    it("renders an empty diff when git_diff_cmd fails", async () => {
      mockTauriCommand("git_status_cmd", () => dirtyStatus);
      mockTauriCommand("git_diff_cmd", () => {
        throw new Error("fatal: bad object");
      });
      const user = userEvent.setup();
      renderWithProviders(<GitPanelContainer />);
      await waitFor(() => {
        expect(screen.getByTestId("git-file-row-a.md")).toBeInTheDocument();
      });
      await user.click(screen.getByTestId("git-file-row-a.md"));
      await waitFor(() => {
        expect(
          screen.getByTestId("git-commit-diff-viewer-empty"),
        ).toBeInTheDocument();
      });
    });

    it("toggles commit selection in the Log list", async () => {
      const user = userEvent.setup();
      renderWithProviders(<GitPanelContainer />);
      await waitFor(() => {
        expect(screen.getByTestId("git-panel-tabs")).toBeInTheDocument();
      });
      await user.click(screen.getByTestId("git-tab-log"));
      const row = await screen.findByTestId("git-log-row-deadbee");
      await user.click(row);
      await waitFor(() =>
        expect(
          screen
            .getByTestId("git-log-row-deadbee")
            .getAttribute("data-selected"),
        ).toBe("true"),
      );
      await user.click(screen.getByTestId("git-log-row-deadbee"));
      await waitFor(() =>
        expect(
          screen
            .getByTestId("git-log-row-deadbee")
            .getAttribute("data-selected"),
        ).toBeNull(),
      );
    });
  });
});
