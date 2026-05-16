import { describe, expect, it, vi } from "vitest";

import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";
import { GitSyncBar } from "@/components/layout/git/GitSyncBar";
import type { UseGitSyncResult } from "@/hooks/useGitSync";

const base: UseGitSyncResult = {
  step: "idle",
  error: null,
  failedStep: null,
  upstreamPrompt: null,
  busy: false,
  sync: vi.fn(),
  confirmSetUpstream: vi.fn(),
  cancelSetUpstream: vi.fn(),
};

describe("GitSyncBar", () => {
  it("fires sync on click when idle", async () => {
    const sync = vi.fn();
    const user = userEvent.setup();
    renderWithProviders(<GitSyncBar {...base} sync={sync} />);
    await user.click(screen.getByTestId("git-sync-button"));
    expect(sync).toHaveBeenCalledTimes(1);
  });

  it("disables the button while busy and shows the step label", () => {
    renderWithProviders(<GitSyncBar {...base} step="pulling" busy />);
    const btn = screen.getByTestId("git-sync-button");
    expect(btn).toBeDisabled();
    expect(btn).toHaveTextContent("Pulling…");
  });

  it("shows which step failed", () => {
    renderWithProviders(
      <GitSyncBar
        {...base}
        step="pushing"
        failedStep="pushing"
        error="rejected"
      />,
    );
    expect(screen.getByTestId("git-sync-error")).toHaveTextContent(
      "pushing failed: rejected",
    );
  });

  it("renders 'up to date' when done", () => {
    renderWithProviders(<GitSyncBar {...base} step="done" />);
    expect(screen.getByTestId("git-sync-done")).toBeInTheDocument();
  });

  it("renders the set-upstream confirm and wires its actions", async () => {
    const confirmSetUpstream = vi.fn();
    const cancelSetUpstream = vi.fn();
    const user = userEvent.setup();
    renderWithProviders(
      <GitSyncBar
        {...base}
        upstreamPrompt={{ branch: "main", remote: "origin" }}
        confirmSetUpstream={confirmSetUpstream}
        cancelSetUpstream={cancelSetUpstream}
      />,
    );
    await user.click(screen.getByTestId("git-sync-upstream-confirm"));
    await user.click(screen.getByTestId("git-sync-upstream-cancel"));
    expect(confirmSetUpstream).toHaveBeenCalledTimes(1);
    expect(cancelSetUpstream).toHaveBeenCalledTimes(1);
  });
});
