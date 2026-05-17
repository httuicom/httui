import { describe, it, expect, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { GitConflictResolver } from "@/components/layout/git/GitConflictResolver";
import type { ConflictVersions } from "@/lib/tauri/git";
import { renderWithProviders, screen } from "@/test/render";

const versions: ConflictVersions = {
  base: "base line\n",
  ours: "ours line\n",
  theirs: "theirs line\n",
};

describe("GitConflictResolver", () => {
  it("renders the resolver header for the path", () => {
    renderWithProviders(
      <GitConflictResolver
        path="auth/login.md"
        versions={versions}
        onResolve={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    const root = screen.getByTestId("git-conflict-resolver");
    expect(root.getAttribute("data-path")).toBe("auth/login.md");
    expect(root.textContent).toContain("Resolving auth/login.md");
  });

  it("toggles the merge base panel", async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <GitConflictResolver
        path="f.md"
        versions={versions}
        onResolve={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    expect(
      screen.queryByTestId("git-conflict-resolver-base"),
    ).not.toBeInTheDocument();
    await user.click(screen.getByTestId("git-conflict-resolver-base-toggle"));
    const base = screen.getByTestId("git-conflict-resolver-base");
    expect(base.textContent).toContain("base line");
  });

  it("shows the add/add note when base is empty", async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <GitConflictResolver
        path="f.md"
        versions={{ base: "", ours: "o", theirs: "t" }}
        onResolve={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    await user.click(screen.getByTestId("git-conflict-resolver-base-toggle"));
    expect(
      screen.getByTestId("git-conflict-resolver-base").textContent,
    ).toContain("no common ancestor");
  });

  it("fires onResolve with the left-pane content", async () => {
    const onResolve = vi.fn();
    const user = userEvent.setup();
    renderWithProviders(
      <GitConflictResolver
        path="f.md"
        versions={versions}
        onResolve={onResolve}
        onCancel={vi.fn()}
      />,
    );
    await user.click(screen.getByTestId("git-conflict-resolver-mark"));
    expect(onResolve).toHaveBeenCalledTimes(1);
    expect(onResolve.mock.calls[0]![0]).toBe("f.md");
    // Left pane seeds from `ours` until the user edits it.
    expect(onResolve.mock.calls[0]![1]).toBe("ours line\n");
  });

  it("fires onCancel", async () => {
    const onCancel = vi.fn();
    const user = userEvent.setup();
    renderWithProviders(
      <GitConflictResolver
        path="f.md"
        versions={versions}
        onResolve={vi.fn()}
        onCancel={onCancel}
      />,
    );
    await user.click(screen.getByTestId("git-conflict-resolver-cancel"));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });
});
