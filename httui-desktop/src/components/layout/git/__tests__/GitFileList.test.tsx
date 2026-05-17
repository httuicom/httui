import { describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { GitFileList } from "@/components/layout/git/GitFileList";
import type { GitFileChange } from "@/lib/tauri/git";
import { renderWithProviders, screen } from "@/test/render";

function fc(over: Partial<GitFileChange> = {}): GitFileChange {
  return {
    path: "a",
    status: "M.",
    staged: false,
    untracked: false,
    ...over,
  };
}

describe("GitFileList", () => {
  it("renders empty hint when changed is empty", () => {
    renderWithProviders(<GitFileList changed={[]} />);
    expect(screen.getByTestId("git-file-list-empty")).toBeInTheDocument();
  });

  it("renders staged / unstaged / untracked groups with counts", () => {
    renderWithProviders(
      <GitFileList
        changed={[
          fc({ path: "s", staged: true }),
          fc({ path: "u" }),
          fc({ path: "t", untracked: true, status: "??" }),
        ]}
      />,
    );
    expect(
      screen.getByTestId("git-file-list-staged").getAttribute("data-count"),
    ).toBe("1");
    expect(
      screen.getByTestId("git-file-list-unstaged").getAttribute("data-count"),
    ).toBe("1");
    expect(
      screen.getByTestId("git-file-list-untracked").getAttribute("data-count"),
    ).toBe("1");
  });

  it("hides groups that have no entries", () => {
    renderWithProviders(<GitFileList changed={[fc({ path: "u" })]} />);
    expect(
      screen.queryByTestId("git-file-list-staged"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByTestId("git-file-list-untracked"),
    ).not.toBeInTheDocument();
    expect(screen.getByTestId("git-file-list-unstaged")).toBeInTheDocument();
  });

  it("encodes per-row status in data-status", () => {
    renderWithProviders(
      <GitFileList
        changed={[
          fc({ path: "m", status: "M." }),
          fc({ path: "d", status: ".D" }),
          fc({ path: "n", untracked: true, status: "??" }),
        ]}
      />,
    );
    expect(
      screen.getByTestId("git-file-row-m").getAttribute("data-status"),
    ).toBe("modified");
    expect(
      screen.getByTestId("git-file-row-d").getAttribute("data-status"),
    ).toBe("deleted");
    expect(
      screen.getByTestId("git-file-row-n").getAttribute("data-status"),
    ).toBe("untracked");
  });

  it("highlights selected row via data-selected", () => {
    renderWithProviders(
      <GitFileList changed={[fc({ path: "a" })]} selectedPath="a" />,
    );
    expect(
      screen.getByTestId("git-file-row-a").getAttribute("data-selected"),
    ).toBe("true");
  });

  it("fires onSelect on row click", async () => {
    const onSelect = vi.fn();
    renderWithProviders(
      <GitFileList changed={[fc({ path: "a" })]} onSelect={onSelect} />,
    );
    await userEvent.setup().click(screen.getByTestId("git-file-row-a"));
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect.mock.calls[0]![0].path).toBe("a");
  });

  it("hides the stage checkbox when no onToggleStage is provided", () => {
    renderWithProviders(<GitFileList changed={[fc({ path: "a" })]} />);
    expect(
      screen.queryByTestId("git-file-row-a-stage"),
    ).not.toBeInTheDocument();
  });

  it("renders the stage checkbox when onToggleStage is provided", () => {
    renderWithProviders(
      <GitFileList changed={[fc({ path: "a" })]} onToggleStage={() => {}} />,
    );
    expect(screen.getByTestId("git-file-row-a-stage")).toBeInTheDocument();
  });

  it("fires onToggleStage when the stage checkbox is clicked", async () => {
    const onToggleStage = vi.fn();
    const onSelect = vi.fn();
    renderWithProviders(
      <GitFileList
        changed={[fc({ path: "a" })]}
        onToggleStage={onToggleStage}
        onSelect={onSelect}
      />,
    );
    await userEvent.setup().click(screen.getByTestId("git-file-row-a-stage"));
    expect(onToggleStage).toHaveBeenCalledTimes(1);
    expect(onToggleStage.mock.calls[0]![0].path).toBe("a");
    // Checkbox click must not bubble up to the row's onSelect.
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("encodes status as 'changed' for an unknown XY code", () => {
    renderWithProviders(
      <GitFileList changed={[fc({ path: "x", status: ".X" })]} />,
    );
    expect(
      screen.getByTestId("git-file-row-x").getAttribute("data-status"),
    ).toBe("changed");
  });

  it("renders an added row for status 'A.'", () => {
    renderWithProviders(
      <GitFileList changed={[fc({ path: "a", status: "A.", staged: true })]} />,
    );
    expect(
      screen.getByTestId("git-file-row-a").getAttribute("data-status"),
    ).toBe("added");
  });

  it("renders a renamed row for status '.R'", () => {
    renderWithProviders(
      <GitFileList changed={[fc({ path: "r", status: ".R" })]} />,
    );
    expect(
      screen.getByTestId("git-file-row-r").getAttribute("data-status"),
    ).toBe("renamed");
  });

  it("renders a conflicted row for status '.U'", () => {
    renderWithProviders(
      <GitFileList changed={[fc({ path: "c", status: ".U" })]} />,
    );
    expect(
      screen.getByTestId("git-file-row-c").getAttribute("data-status"),
    ).toBe("conflicted");
  });

  it("does not fire onSelect when no callback is provided", async () => {
    renderWithProviders(<GitFileList changed={[fc({ path: "a" })]} />);
    // Should not crash without onSelect.
    await userEvent.setup().click(screen.getByTestId("git-file-row-a"));
    expect(screen.getByTestId("git-file-row-a")).toBeInTheDocument();
  });
});
