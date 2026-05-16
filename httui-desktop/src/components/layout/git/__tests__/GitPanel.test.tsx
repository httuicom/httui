import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import {
  GitPanel,
  type GitPanelProps,
  type GitPanelTab,
} from "@/components/layout/git/GitPanel";
import type { CommitInfo, GitStatus } from "@/lib/tauri/git";
import { renderWithProviders, screen } from "@/test/render";

function status(over: Partial<GitStatus> = {}): GitStatus {
  return {
    branch: "main",
    upstream: "origin/main",
    ahead: 0,
    behind: 0,
    changed: [],
    clean: true,
    ...over,
  };
}

function commit(over: Partial<CommitInfo> = {}): CommitInfo {
  return {
    sha: "deadbeef0000000000000000000000000000aaaa",
    short_sha: "deadbee",
    author_name: "Jane Doe",
    author_email: "jane@x.test",
    timestamp: Math.floor(Date.now() / 1000) - 30,
    subject: "first commit",
    ...over,
  };
}

/** Controlled component — harness owns the active-tab state so a
 * click actually flips the rendered section. */
function Harness(props: Omit<GitPanelProps, "activeTab" | "onSelectTab">) {
  const [tab, setTab] = useState<GitPanelTab>("status");
  return <GitPanel {...props} activeTab={tab} onSelectTab={setTab} />;
}

describe("GitPanel", () => {
  it("shows loading state when status is null", () => {
    renderWithProviders(<GitPanel status={null} commits={[]} />);
    expect(screen.getByTestId("git-panel").getAttribute("data-loading")).toBe(
      "true",
    );
  });

  it("renders the three tabs when status resolves", () => {
    renderWithProviders(<GitPanel status={status()} commits={[commit()]} />);
    expect(screen.getByTestId("git-panel-tabs")).toBeInTheDocument();
    expect(screen.getByTestId("git-tab-status")).toBeInTheDocument();
    expect(screen.getByTestId("git-tab-log")).toBeInTheDocument();
    expect(screen.getByTestId("git-tab-audit")).toBeInTheDocument();
  });

  it("flags clean working tree via data-clean", () => {
    renderWithProviders(<GitPanel status={status()} commits={[]} />);
    expect(screen.getByTestId("git-panel").getAttribute("data-clean")).toBe(
      "true",
    );
  });

  it("defaults to the Status tab — header + working tree", () => {
    renderWithProviders(<GitPanel status={status()} commits={[commit()]} />);
    expect(screen.getByTestId("git-status-header")).toBeInTheDocument();
    expect(
      screen.getByTestId("git-panel-section-working-tree"),
    ).toBeInTheDocument();
    expect(
      screen.queryByTestId("git-panel-section-log"),
    ).not.toBeInTheDocument();
  });

  it("forwards changed files into the file list (Status tab)", () => {
    renderWithProviders(
      <GitPanel
        status={status({
          clean: false,
          changed: [
            { path: "a", status: "M.", staged: false, untracked: false },
          ],
        })}
        commits={[]}
      />,
    );
    expect(screen.getByTestId("git-file-row-a")).toBeInTheDocument();
  });

  it("switches to the Log tab and shows commits", async () => {
    const user = userEvent.setup();
    renderWithProviders(<Harness status={status()} commits={[commit()]} />);
    await user.click(screen.getByTestId("git-tab-log"));
    expect(screen.getByTestId("git-panel-section-log")).toBeInTheDocument();
    expect(screen.getByTestId("git-log-row-deadbee")).toBeInTheDocument();
    expect(
      screen.queryByTestId("git-panel-section-working-tree"),
    ).not.toBeInTheDocument();
  });

  it("switches to the Audit tab — log, no action-type filters", async () => {
    const user = userEvent.setup();
    renderWithProviders(<Harness status={status()} commits={[commit()]} />);
    await user.click(screen.getByTestId("git-tab-audit"));
    expect(screen.getByTestId("git-panel-section-audit")).toBeInTheDocument();
    expect(screen.getByTestId("git-log-row-deadbee")).toBeInTheDocument();
  });

  it("fires onSelectTab on tab click", async () => {
    const user = userEvent.setup();
    const onSelectTab = vi.fn();
    renderWithProviders(
      <GitPanel
        status={status()}
        commits={[]}
        activeTab="status"
        onSelectTab={onSelectTab}
      />,
    );
    await user.click(screen.getByTestId("git-tab-log"));
    expect(onSelectTab).toHaveBeenCalledWith("log");
  });

  it("renders empty file list and empty log list", async () => {
    const user = userEvent.setup();
    renderWithProviders(<Harness status={status()} commits={[]} />);
    expect(screen.getByTestId("git-file-list-empty")).toBeInTheDocument();
    await user.click(screen.getByTestId("git-tab-log"));
    expect(screen.getByTestId("git-log-list-empty")).toBeInTheDocument();
  });

  describe("commit form + diff (cenário 2)", () => {
    const commitProps = {
      stagedCount: 2,
      commitMessage: "",
      commitAmend: false,
      onCommitMessageChange: vi.fn(),
      onCommitAmendChange: vi.fn(),
      onCommit: vi.fn(),
    };

    it("renders the commit form on the Status tab when wired", () => {
      renderWithProviders(
        <GitPanel status={status()} commits={[]} {...commitProps} />,
      );
      expect(screen.getByTestId("git-commit-form")).toBeInTheDocument();
    });

    it("omits the commit form when handlers are absent", () => {
      renderWithProviders(<GitPanel status={status()} commits={[]} />);
      expect(
        screen.queryByTestId("git-commit-form"),
      ).not.toBeInTheDocument();
    });

    it("hides the diff inspector when diff is undefined", () => {
      renderWithProviders(
        <GitPanel status={status()} commits={[]} {...commitProps} />,
      );
      expect(
        screen.queryByTestId("git-panel-section-diff"),
      ).not.toBeInTheDocument();
    });

    it("shows the diff inspector loading state when diff is null", () => {
      renderWithProviders(
        <GitPanel
          status={status()}
          commits={[]}
          {...commitProps}
          diff={null}
        />,
      );
      expect(
        screen.getByTestId("git-panel-section-diff"),
      ).toBeInTheDocument();
      expect(
        screen.getByTestId("git-commit-diff-viewer").getAttribute(
          "data-loading",
        ),
      ).toBe("true");
    });

    it("renders the diff text when diff is a string", () => {
      renderWithProviders(
        <GitPanel
          status={status()}
          commits={[]}
          {...commitProps}
          diff={"diff --git a/x b/x\n+added"}
          diffSubject="Working tree changes"
        />,
      );
      expect(
        screen.getByTestId("git-commit-diff-viewer"),
      ).toBeInTheDocument();
      expect(screen.getByText("+added")).toBeInTheDocument();
    });
  });

  describe("log filter + diff on Log tab (cenário 3)", () => {
    it("renders the log filter on the Log tab when wired", () => {
      renderWithProviders(
        <GitPanel
          status={status()}
          commits={[commit()]}
          activeTab="log"
          logFilter={{ mode: "author", query: "" }}
          onLogFilterChange={vi.fn()}
        />,
      );
      expect(screen.getByTestId("git-log-filter")).toBeInTheDocument();
    });

    it("omits the log filter when not wired", () => {
      renderWithProviders(
        <GitPanel
          status={status()}
          commits={[commit()]}
          activeTab="log"
        />,
      );
      expect(
        screen.queryByTestId("git-log-filter"),
      ).not.toBeInTheDocument();
    });

    it("shows the commit diff inspector on the Log tab", () => {
      renderWithProviders(
        <GitPanel
          status={status()}
          commits={[commit()]}
          activeTab="log"
          selectedCommitSha={commit().sha}
          diff={"diff --git a/x b/x\n+log line"}
          diffShortSha="deadbee"
          diffSubject="first commit"
        />,
      );
      expect(
        screen.getByTestId("git-panel-section-diff"),
      ).toBeInTheDocument();
      expect(screen.getByText("+log line")).toBeInTheDocument();
    });
  });

  describe("sync toolbar + upstream prompt (cenário 5)", () => {
    it("renders the sync buttons when handlers are wired", () => {
      renderWithProviders(
        <GitPanel
          status={status()}
          commits={[]}
          onFetch={vi.fn()}
          onPull={vi.fn()}
          onPush={vi.fn()}
        />,
      );
      expect(screen.getByTestId("git-sync-buttons")).toBeInTheDocument();
    });

    it("omits the sync toolbar when no sync handlers", () => {
      renderWithProviders(<GitPanel status={status()} commits={[]} />);
      expect(
        screen.queryByTestId("git-sync-buttons"),
      ).not.toBeInTheDocument();
    });

    it("shows the upstream prompt and fires confirm/cancel", async () => {
      const onConfirm = vi.fn();
      const onCancel = vi.fn();
      const user = userEvent.setup();
      renderWithProviders(
        <GitPanel
          status={status()}
          commits={[]}
          onPush={vi.fn()}
          upstreamPrompt={{ branch: "feat/x", remote: "origin" }}
          onConfirmSetUpstream={onConfirm}
          onCancelSetUpstream={onCancel}
        />,
      );
      const prompt = screen.getByTestId("git-upstream-prompt");
      expect(prompt.textContent).toContain("feat/x");
      expect(prompt.textContent).toContain("origin/feat/x");
      await user.click(
        screen.getByTestId("git-upstream-prompt-confirm"),
      );
      expect(onConfirm).toHaveBeenCalledTimes(1);
      await user.click(screen.getByTestId("git-upstream-prompt-cancel"));
      expect(onCancel).toHaveBeenCalledTimes(1);
    });

    it("disables Push when there is no remote", () => {
      renderWithProviders(
        <GitPanel
          status={status()}
          commits={[]}
          onPush={vi.fn()}
          hasRemote={false}
        />,
      );
      const pushBtn = screen.getByTestId(
        "git-sync-push",
      ) as HTMLButtonElement;
      expect(pushBtn.disabled).toBe(true);
      expect(
        screen.getByTestId("git-sync-no-remote-hint"),
      ).toBeInTheDocument();
    });
  });

  describe("conflict resolution (cenário 6)", () => {
    it("renders the conflict banner on the Status tab", () => {
      renderWithProviders(
        <GitPanel
          status={status({ clean: false })}
          commits={[]}
          conflicts={["a.md", "b.md"]}
          onOpenConflict={vi.fn()}
          onAcceptYours={vi.fn()}
          onAcceptTheirs={vi.fn()}
        />,
      );
      expect(
        screen.getByTestId("git-conflict-banner"),
      ).toBeInTheDocument();
      expect(
        screen.getByTestId("git-conflict-row-a.md"),
      ).toBeInTheDocument();
    });

    it("omits the banner when there are no conflicts", () => {
      renderWithProviders(
        <GitPanel status={status()} commits={[]} conflicts={[]} />,
      );
      expect(
        screen.queryByTestId("git-conflict-banner"),
      ).not.toBeInTheDocument();
    });

    it("takes over the body with the resolver when active", () => {
      renderWithProviders(
        <GitPanel
          status={status()}
          commits={[]}
          resolver={{
            path: "a.md",
            versions: { base: "b", ours: "o", theirs: "t" },
          }}
          onResolveMerged={vi.fn()}
          onCancelResolver={vi.fn()}
        />,
      );
      expect(screen.getByTestId("git-panel-resolver")).toBeInTheDocument();
      expect(
        screen.getByTestId("git-conflict-resolver"),
      ).toBeInTheDocument();
      // Tab bodies are suppressed while resolving.
      expect(
        screen.queryByTestId("git-panel-section-working-tree"),
      ).not.toBeInTheDocument();
    });
  });
});
