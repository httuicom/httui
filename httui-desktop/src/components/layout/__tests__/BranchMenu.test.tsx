import { describe, it, expect, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { BranchMenu } from "@/components/layout/BranchMenu";
import type { BranchInfo } from "@/lib/tauri/git";
import { renderWithProviders, screen } from "@/test/render";

const BRANCHES: BranchInfo[] = [
  { name: "main", current: true, remote: false },
  { name: "feat/x", current: false, remote: false },
];

describe("BranchMenu", () => {
  it("renders the dash placeholder when no branch is given", () => {
    renderWithProviders(<BranchMenu branch={null} />);
    expect(screen.getByTestId("status-branch").textContent).toBe("—");
  });

  it("renders the branch label on the trigger", () => {
    renderWithProviders(<BranchMenu branch="feat/login" />);
    expect(
      screen.getByRole("button", { name: /Branch feat\/login/ }),
    ).toBeInTheDocument();
    expect(screen.getByTestId("status-branch").textContent).toBe("feat/login");
  });

  it("hides the counts cell when every category is zero", () => {
    renderWithProviders(<BranchMenu branch="main" />);
    expect(screen.queryByTestId("status-changes")).toBeNull();
  });

  it("renders ahead/behind + add/modify/delete counts when nonzero", () => {
    renderWithProviders(
      <BranchMenu
        branch="main"
        ahead={2}
        behind={1}
        added={3}
        modified={5}
        deleted={1}
      />,
    );
    const counts = screen.getByTestId("status-changes");
    expect(counts.textContent).toContain("↑2");
    expect(counts.textContent).toContain("↓1");
    expect(counts.textContent).toContain("+3");
    expect(counts.textContent).toContain("~5");
    expect(counts.textContent).toContain("-1");
  });

  it("only renders nonzero categories", () => {
    renderWithProviders(<BranchMenu branch="main" added={2} />);
    const counts = screen.getByTestId("status-changes");
    expect(counts.textContent).toContain("+2");
    expect(counts.textContent).not.toContain("↑");
    expect(counts.textContent).not.toContain("↓");
    expect(counts.textContent).not.toContain("~");
    expect(counts.textContent).not.toContain("-");
  });

  it("opens a placeholder dropdown on click", async () => {
    const user = userEvent.setup();
    renderWithProviders(<BranchMenu branch="main" />);

    await user.click(screen.getByRole("button", { name: /Branch main/ }));

    expect(screen.getByTestId("branch-menu-placeholder")).toBeInTheDocument();
    // Mentions V10 so users know what's coming.
    expect(screen.getByTestId("branch-menu-placeholder").textContent).toMatch(
      /V10/,
    );
  });

  describe("switcher mode", () => {
    it("renders the branch picker instead of the placeholder", async () => {
      const user = userEvent.setup();
      renderWithProviders(
        <BranchMenu
          branch="main"
          branches={BRANCHES}
          onSelectBranch={vi.fn()}
          onCreateBranch={vi.fn()}
        />,
      );
      await user.click(screen.getByRole("button", { name: /Branch main/ }));
      expect(screen.getByTestId("branch-menu-switcher")).toBeInTheDocument();
      expect(screen.getByTestId("git-branch-picker")).toBeInTheDocument();
      expect(
        screen.queryByTestId("branch-menu-placeholder"),
      ).not.toBeInTheDocument();
    });

    it("calls onMenuOpen when the dropdown opens", async () => {
      const onMenuOpen = vi.fn();
      const user = userEvent.setup();
      renderWithProviders(
        <BranchMenu
          branch="main"
          branches={BRANCHES}
          onMenuOpen={onMenuOpen}
          onSelectBranch={vi.fn()}
        />,
      );
      await user.click(screen.getByRole("button", { name: /Branch main/ }));
      expect(onMenuOpen).toHaveBeenCalled();
    });

    it("fires onSelectBranch when a non-current branch is clicked", async () => {
      const onSelectBranch = vi.fn();
      const user = userEvent.setup();
      renderWithProviders(
        <BranchMenu
          branch="main"
          branches={BRANCHES}
          onSelectBranch={onSelectBranch}
        />,
      );
      await user.click(screen.getByRole("button", { name: /Branch main/ }));
      await user.click(screen.getByTestId("git-branch-picker-row-feat/x"));
      expect(onSelectBranch).toHaveBeenCalledWith(BRANCHES[1]);
    });
  });
});
