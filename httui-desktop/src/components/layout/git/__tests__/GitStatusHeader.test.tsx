import { describe, expect, it } from "vitest";

import { GitStatusHeader } from "@/components/layout/git/GitStatusHeader";
import type { GitStatus } from "@/lib/tauri/git";
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

describe("GitStatusHeader", () => {
  it("renders branch label, upstream, and clean tag when there are no changes", () => {
    renderWithProviders(<GitStatusHeader status={status()} />);
    expect(screen.getByTestId("git-status-header-branch").textContent).toBe(
      "main",
    );
    expect(screen.getByTestId("git-status-header-upstream").textContent).toBe(
      "→ origin/main",
    );
    expect(screen.getByTestId("git-status-header-clean")).toBeInTheDocument();
  });

  it("hides upstream chip when upstream is null and flags noUpstream", () => {
    renderWithProviders(
      <GitStatusHeader status={status({ upstream: null })} />,
    );
    expect(
      screen.queryByTestId("git-status-header-upstream"),
    ).not.toBeInTheDocument();
    expect(
      screen.getByTestId("git-status-header").getAttribute("data-no-upstream"),
    ).toBe("true");
  });

  it("renders ahead and behind chips when counts are non-zero", () => {
    renderWithProviders(
      <GitStatusHeader status={status({ ahead: 3, behind: 2 })} />,
    );
    expect(screen.getByTestId("git-status-header-ahead").textContent).toMatch(
      /↑\s*3/,
    );
    expect(screen.getByTestId("git-status-header-behind").textContent).toMatch(
      /↓\s*2/,
    );
  });

  it("renders dirty count with sing/plural agreement", () => {
    renderWithProviders(
      <GitStatusHeader
        status={status({
          clean: false,
          changed: [
            { path: "a", status: "M.", staged: false, untracked: false },
          ],
        })}
      />,
    );
    expect(screen.getByTestId("git-status-header-dirty").textContent).toBe(
      "1 change",
    );

    renderWithProviders(
      <GitStatusHeader
        status={status({
          clean: false,
          changed: [
            { path: "a", status: "M.", staged: false, untracked: false },
            { path: "b", status: "M.", staged: false, untracked: false },
          ],
        })}
      />,
    );
    const dirtyEls = screen.getAllByTestId("git-status-header-dirty");
    expect(dirtyEls[dirtyEls.length - 1]!.textContent).toBe("2 changes");
  });

  it("flags detached state when branch is null", () => {
    renderWithProviders(<GitStatusHeader status={status({ branch: null })} />);
    expect(screen.getByTestId("git-status-header-branch").textContent).toBe(
      "(detached)",
    );
    expect(
      screen.getByTestId("git-status-header").getAttribute("data-detached"),
    ).toBe("true");
  });
});
