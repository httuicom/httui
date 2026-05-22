import { describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { GitBranchPicker } from "@/components/layout/git/GitBranchPicker";
import type { BranchInfo } from "@/lib/tauri/git";
import { renderWithProviders, screen } from "@/test/render";

function branch(over: Partial<BranchInfo> = {}): BranchInfo {
  return {
    name: "main",
    current: false,
    remote: false,
    ...over,
  };
}

describe("GitBranchPicker", () => {
  it("renders empty hint when filter matches nothing", async () => {
    renderWithProviders(
      <GitBranchPicker branches={[branch({ name: "main" })]} />,
    );
    await userEvent
      .setup()
      .type(screen.getByTestId("git-branch-picker-filter"), "nope");
    expect(screen.getByTestId("git-branch-picker-empty")).toBeInTheDocument();
  });

  it("groups branches into Local and Remote sections", () => {
    renderWithProviders(
      <GitBranchPicker
        branches={[
          branch({ name: "main", current: true }),
          branch({ name: "feat/x" }),
          branch({ name: "origin/main", remote: true }),
        ]}
      />,
    );
    expect(screen.getByTestId("git-branch-picker-local")).toBeInTheDocument();
    expect(screen.getByTestId("git-branch-picker-remote")).toBeInTheDocument();
  });

  it("hides the Remote section when no remote branches", () => {
    renderWithProviders(
      <GitBranchPicker branches={[branch({ name: "main", current: true })]} />,
    );
    expect(
      screen.queryByTestId("git-branch-picker-remote"),
    ).not.toBeInTheDocument();
  });

  it("flags the current branch and renders it as inert", () => {
    renderWithProviders(
      <GitBranchPicker
        branches={[branch({ name: "main", current: true })]}
        onSelectBranch={() => {}}
      />,
    );
    const row = screen.getByTestId("git-branch-picker-row-main");
    expect(row.getAttribute("data-current")).toBe("true");
    expect(row.tagName).toBe("DIV");
  });

  it("makes non-current branches buttons that fire onSelectBranch", async () => {
    const onSelectBranch = vi.fn();
    renderWithProviders(
      <GitBranchPicker
        branches={[
          branch({ name: "main", current: true }),
          branch({ name: "feat/x" }),
        ]}
        onSelectBranch={onSelectBranch}
      />,
    );
    const row = screen.getByTestId("git-branch-picker-row-feat/x");
    expect(row.tagName).toBe("BUTTON");
    await userEvent.setup().click(row);
    expect(onSelectBranch).toHaveBeenCalledWith(
      expect.objectContaining({ name: "feat/x" }),
    );
  });

  it("filters branches case-insensitively as the user types", async () => {
    renderWithProviders(
      <GitBranchPicker
        branches={[branch({ name: "main" }), branch({ name: "feat/payments" })]}
      />,
    );
    await userEvent
      .setup()
      .type(screen.getByTestId("git-branch-picker-filter"), "PAY");
    expect(
      screen.queryByTestId("git-branch-picker-row-main"),
    ).not.toBeInTheDocument();
    expect(
      screen.getByTestId("git-branch-picker-row-feat/payments"),
    ).toBeInTheDocument();
  });

  it("hides + new branch button when no onCreateBranch is supplied", () => {
    renderWithProviders(
      <GitBranchPicker branches={[branch({ name: "main", current: true })]} />,
    );
    expect(
      screen.queryByTestId("git-branch-picker-new"),
    ).not.toBeInTheDocument();
  });

  it("opens the create form on + new click and fires onCreateBranch on submit", async () => {
    const onCreateBranch = vi.fn();
    renderWithProviders(
      <GitBranchPicker
        branches={[branch({ name: "main", current: true })]}
        onCreateBranch={onCreateBranch}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("git-branch-picker-new"));
    expect(
      screen.getByTestId("git-branch-picker-create-form"),
    ).toBeInTheDocument();
    await user.type(
      screen.getByTestId("git-branch-picker-create-input"),
      "feat/new",
    );
    await user.click(screen.getByTestId("git-branch-picker-create-submit"));
    expect(onCreateBranch).toHaveBeenCalledWith("feat/new");
    expect(
      screen.queryByTestId("git-branch-picker-create-form"),
    ).not.toBeInTheDocument();
  });

  it("disables submit + create button when name is empty / whitespace", async () => {
    renderWithProviders(
      <GitBranchPicker
        branches={[branch({ name: "main", current: true })]}
        onCreateBranch={() => {}}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("git-branch-picker-new"));
    const submit = screen.getByTestId(
      "git-branch-picker-create-submit",
    ) as HTMLButtonElement;
    expect(submit.disabled).toBe(true);
    await user.type(
      screen.getByTestId("git-branch-picker-create-input"),
      "   ",
    );
    expect(submit.disabled).toBe(true);
  });

  it("cancel button closes the create form without firing onCreateBranch", async () => {
    const onCreateBranch = vi.fn();
    renderWithProviders(
      <GitBranchPicker
        branches={[branch({ name: "main", current: true })]}
        onCreateBranch={onCreateBranch}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("git-branch-picker-new"));
    await user.click(screen.getByTestId("git-branch-picker-create-cancel"));
    expect(onCreateBranch).not.toHaveBeenCalled();
    expect(
      screen.queryByTestId("git-branch-picker-create-form"),
    ).not.toBeInTheDocument();
  });

  it("disables interactions when busy", () => {
    renderWithProviders(
      <GitBranchPicker
        branches={[branch({ name: "feat/x" })]}
        onSelectBranch={() => {}}
        busy
      />,
    );
    const filter = screen.getByTestId(
      "git-branch-picker-filter",
    ) as HTMLInputElement;
    expect(filter.disabled).toBe(true);
    const row = screen.getByTestId("git-branch-picker-row-feat/x");
    expect(row.tagName).toBe("DIV");
    expect(
      screen.getByTestId("git-branch-picker").getAttribute("data-busy"),
    ).toBe("true");
  });

  it("Enter key in the create input submits", async () => {
    const onCreateBranch = vi.fn();
    renderWithProviders(
      <GitBranchPicker
        branches={[branch({ name: "main", current: true })]}
        onCreateBranch={onCreateBranch}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("git-branch-picker-new"));
    await user.type(
      screen.getByTestId("git-branch-picker-create-input"),
      "feat/x{Enter}",
    );
    expect(onCreateBranch).toHaveBeenCalledWith("feat/x");
  });
});
