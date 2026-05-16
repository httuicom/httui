import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { fireEvent, renderWithProviders, screen, waitFor } from "@/test/render";
import { clearTauriMocks, mockTauriCommand } from "@/test/mocks/tauri";
import { useWorkspaceStore } from "@/stores/workspace";
import { useSettingsStore } from "@/stores/settings";
import { usePaneStore } from "@/stores/pane";
import { GitSidePanel } from "@/components/layout/git/GitSidePanel";

const dirty = (path: string) => ({
  path,
  status: " M",
  staged: false,
  untracked: false,
});
const statusWith = (paths: string[]) => ({
  branch: "main",
  upstream: null,
  ahead: 0,
  behind: 0,
  clean: paths.length === 0,
  changed: paths.map(dirty),
});

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
  useSettingsStore.setState({ gitCommitTemplate: "" });
  mockTauriCommand("git_status_cmd", () => SAMPLE);
  mockTauriCommand("git_remote_list_cmd", () => []);
});

afterEach(() => {
  clearTauriMocks();
  useWorkspaceStore.setState({ vaultPath: null });
  useSettingsStore.setState({ gitCommitTemplate: "" });
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

  describe("commit message prefill (cenário 2 + 8)", () => {
    it("prefills with the single changed note", async () => {
      mockTauriCommand("git_status_cmd", () =>
        statusWith(["notes/rollout.md"]),
      );
      renderWithProviders(<GitSidePanel width={340} onClose={() => {}} />);
      const ta = await screen.findByTestId("git-commit-form-message");
      await waitFor(() => expect(ta).toHaveValue("Update rollout"));
    });

    it("prefills with the note count when several changed", async () => {
      mockTauriCommand("git_status_cmd", () =>
        statusWith(["a.md", "b.md", "c.md"]),
      );
      renderWithProviders(<GitSidePanel width={340} onClose={() => {}} />);
      const ta = await screen.findByTestId("git-commit-form-message");
      await waitFor(() => expect(ta).toHaveValue("Update 3 notes"));
    });

    it("honors a configured commit template", async () => {
      useSettingsStore.setState({
        gitCommitTemplate: "docs: {{notes}} ({{count}})",
      });
      mockTauriCommand("git_status_cmd", () => statusWith(["x.md"]));
      renderWithProviders(<GitSidePanel width={340} onClose={() => {}} />);
      const ta = await screen.findByTestId("git-commit-form-message");
      await waitFor(() => expect(ta).toHaveValue("docs: x (1)"));
    });

    it("a hand-edit wins, then clearing falls back to the template", async () => {
      mockTauriCommand("git_status_cmd", () => statusWith(["foo.md"]));
      renderWithProviders(<GitSidePanel width={340} onClose={() => {}} />);
      const ta = await screen.findByTestId("git-commit-form-message");
      await waitFor(() => expect(ta).toHaveValue("Update foo"));

      fireEvent.change(ta, { target: { value: "custom message" } });
      expect(ta).toHaveValue("custom message");

      fireEvent.change(ta, { target: { value: "" } });
      await waitFor(() => expect(ta).toHaveValue("Update foo"));
    });
  });

  describe("changed files (cenário 5)", () => {
    it("lists changed files with their status glyph", async () => {
      mockTauriCommand("git_status_cmd", () => statusWith(["notes/a.md"]));
      renderWithProviders(<GitSidePanel width={340} onClose={() => {}} />);
      const row = await screen.findByTestId("git-file-row-notes/a.md");
      expect(row).toHaveAttribute("data-status", "modified");
    });

    it("mounts the Sync bar (cenário 3)", async () => {
      mockTauriCommand("git_status_cmd", () => statusWith(["foo.md"]));
      renderWithProviders(<GitSidePanel width={340} onClose={() => {}} />);
      expect(await screen.findByTestId("git-sync-button")).toBeInTheDocument();
    });

    it("staging a row routes through stage_path_cmd", async () => {
      mockTauriCommand("git_status_cmd", () => statusWith(["foo.md"]));
      let staged: string | null = null;
      mockTauriCommand("stage_path_cmd", (a) => {
        staged = (a as { path: string }).path;
      });
      const user = userEvent.setup();
      renderWithProviders(<GitSidePanel width={340} onClose={() => {}} />);
      const cb = await screen.findByTestId("git-file-row-foo.md-stage");
      await user.click(cb);
      await waitFor(() => expect(staged).toBe("foo.md"));
    });
  });
});
