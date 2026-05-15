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
});
