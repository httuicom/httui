import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { renderWithProviders, screen } from "@/test/render";
import { clearTauriMocks, mockTauriCommand } from "@/test/mocks/tauri";
import { useWorkspaceStore } from "@/stores/workspace";
import { usePaneStore } from "@/stores/pane";
import { GitSidePanel } from "@/components/layout/git/GitSidePanel";

const SAMPLE = {
  branch: "main",
  upstream: "origin/main",
  ahead: 1,
  behind: 0,
  changed: [],
  clean: true,
};

beforeEach(() => {
  clearTauriMocks();
  useWorkspaceStore.setState({ vaultPath: "/v" });
  mockTauriCommand("git_status_cmd", () => SAMPLE);
  mockTauriCommand("git_remote_list_cmd", () => []);
});

afterEach(() => {
  clearTauriMocks();
  useWorkspaceStore.setState({ vaultPath: null });
});

describe("GitSidePanel", () => {
  it("renders the Source Control header", () => {
    renderWithProviders(<GitSidePanel width={340} onClose={() => {}} />);
    expect(screen.getByText("Source Control")).toBeInTheDocument();
    expect(screen.getByTestId("git-side-panel")).toBeInTheDocument();
  });

  it("shows the shared git status once it loads", async () => {
    renderWithProviders(<GitSidePanel width={340} onClose={() => {}} />);
    expect(await screen.findByTestId("git-status-header")).toBeInTheDocument();
    expect(screen.getByTestId("git-status-header-branch")).toHaveTextContent(
      "main",
    );
  });

  it("Details opens the detailed pane-tab", async () => {
    const openGitTab = vi.fn();
    usePaneStore.setState({ openGitTab });
    const user = userEvent.setup();
    renderWithProviders(<GitSidePanel width={340} onClose={() => {}} />);
    await user.click(screen.getByTestId("git-side-panel-details"));
    expect(openGitTab).toHaveBeenCalledTimes(1);
  });

  it("the close button calls onClose", async () => {
    const onClose = vi.fn();
    const user = userEvent.setup();
    renderWithProviders(<GitSidePanel width={340} onClose={onClose} />);
    await user.click(
      screen.getByRole("button", { name: "Close git side panel" }),
    );
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("renders an empty state when no vault is open", () => {
    useWorkspaceStore.setState({ vaultPath: null });
    renderWithProviders(<GitSidePanel width={340} onClose={() => {}} />);
    expect(screen.getByTestId("git-side-panel-empty")).toHaveTextContent(
      "No vault open",
    );
  });

  it("is not a Dialog — keeps CM6 focus (no focus trap)", () => {
    renderWithProviders(<GitSidePanel width={340} onClose={() => {}} />);
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
