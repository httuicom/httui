import { describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { GitLogList } from "@/components/layout/git/GitLogList";
import type { CommitInfo } from "@/lib/tauri/git";
import { renderWithProviders, screen } from "@/test/render";

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

describe("GitLogList", () => {
  it("renders empty hint when commits is empty", () => {
    renderWithProviders(<GitLogList commits={[]} />);
    expect(screen.getByTestId("git-log-list-empty")).toBeInTheDocument();
  });

  it("renders one row per commit with short_sha + initials + subject", () => {
    renderWithProviders(
      <GitLogList
        commits={[commit(), commit({ short_sha: "f00ba12", subject: "next" })]}
      />,
    );
    expect(screen.getByTestId("git-log-list").getAttribute("data-count")).toBe(
      "2",
    );
    expect(screen.getByTestId("git-log-row-deadbee-initials").textContent).toBe(
      "JD",
    );
    expect(screen.getByTestId("git-log-row-f00ba12")).toBeInTheDocument();
  });

  it("highlights selected commit via data-selected", () => {
    renderWithProviders(
      <GitLogList
        commits={[commit({ short_sha: "abc1234" })]}
        selectedSha={commit({ short_sha: "abc1234" }).sha}
      />,
    );
    expect(
      screen.getByTestId("git-log-row-abc1234").getAttribute("data-selected"),
    ).toBe("true");
  });

  it("fires onSelect with the commit on row click", async () => {
    const onSelect = vi.fn();
    renderWithProviders(
      <GitLogList commits={[commit()]} onSelect={onSelect} />,
    );
    await userEvent.setup().click(screen.getByTestId("git-log-row-deadbee"));
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect.mock.calls[0]![0].short_sha).toBe("deadbee");
  });

  it("renders relative time as '<N>s ago' for recent commits", () => {
    renderWithProviders(
      <GitLogList
        commits={[commit({ timestamp: Math.floor(Date.now() / 1000) - 10 })]}
      />,
    );
    expect(screen.getByTestId("git-log-row-deadbee").textContent).toMatch(
      /\ds ago/,
    );
  });
});
