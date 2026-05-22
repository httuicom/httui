import { describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { CommitChangelog } from "@/components/layout/git/CommitChangelog";
import { renderWithProviders, screen } from "@/test/render";

describe("CommitChangelog", () => {
  it("renders header with title + tab kbd hint", () => {
    renderWithProviders(<CommitChangelog entries={[]} />);
    expect(screen.getByTestId("commit-changelog-title").textContent).toBe(
      "Auto-generated changelog",
    );
    expect(screen.getByTestId("commit-changelog-tab-hint").textContent).toBe(
      "tab",
    );
  });

  it("shows empty hint when entries is [] and not loading or errored", () => {
    renderWithProviders(<CommitChangelog entries={[]} />);
    expect(screen.getByTestId("commit-changelog-empty").textContent).toMatch(
      /no block-level changes/i,
    );
    expect(
      screen.getByTestId("commit-changelog").getAttribute("data-state"),
    ).toBe("empty");
  });

  it("shows loading hint when loading is true (preempts empty)", () => {
    renderWithProviders(<CommitChangelog entries={[]} loading />);
    expect(screen.getByTestId("commit-changelog-loading").textContent).toMatch(
      /generating changelog/i,
    );
    expect(
      screen.getByTestId("commit-changelog").getAttribute("data-state"),
    ).toBe("loading");
  });

  it("shows error message and replaces body when error is set (preempts loading)", () => {
    renderWithProviders(
      <CommitChangelog
        entries={[]}
        loading
        error="Sidecar offline — try again"
      />,
    );
    expect(screen.getByTestId("commit-changelog-error").textContent).toBe(
      "Sidecar offline — try again",
    );
    expect(
      screen.queryByTestId("commit-changelog-loading"),
    ).not.toBeInTheDocument();
    expect(
      screen.getByTestId("commit-changelog").getAttribute("data-state"),
    ).toBe("error");
  });

  it("renders entries with monospaced block id and description", () => {
    renderWithProviders(
      <CommitChangelog
        entries={[
          { blockId: "b06", text: "POST → PATCH + Authorization" },
          { blockId: "b08", text: "novo bloco SQL para 50 rotas" },
        ]}
      />,
    );
    const rows = screen.getAllByTestId("commit-changelog-row");
    expect(rows).toHaveLength(2);
    const ids = screen.getAllByTestId("commit-changelog-row-id");
    expect(ids[0]!.textContent).toBe("b06:");
    expect(ids[1]!.textContent).toBe("b08:");
    expect(rows[0]!.getAttribute("data-block-id")).toBe("b06");
    expect(rows[0]!.tagName).toBe("LI");
    expect(
      screen.getByTestId("commit-changelog").getAttribute("data-state"),
    ).toBe("ready");
  });

  it("rows become buttons when onAccept is supplied and fire with the entry", async () => {
    const onAccept = vi.fn();
    const entries = [{ blockId: "b06", text: "POST → PATCH" }];
    renderWithProviders(
      <CommitChangelog entries={entries} onAccept={onAccept} />,
    );
    const row = screen.getByTestId("commit-changelog-row");
    expect(row.tagName).toBe("BUTTON");
    await userEvent.setup().click(row);
    expect(onAccept).toHaveBeenCalledTimes(1);
    expect(onAccept).toHaveBeenCalledWith(entries[0]);
  });

  it("dismiss button only renders when onDismiss is set, and fires onDismiss", async () => {
    const { rerender } = renderWithProviders(<CommitChangelog entries={[]} />);
    expect(
      screen.queryByTestId("commit-changelog-dismiss"),
    ).not.toBeInTheDocument();

    const onDismiss = vi.fn();
    rerender(<CommitChangelog entries={[]} onDismiss={onDismiss} />);
    const dismiss = screen.getByTestId("commit-changelog-dismiss");
    expect(dismiss.getAttribute("aria-label")).toBe("Dismiss AI changelog");
    await userEvent.setup().click(dismiss);
    expect(onDismiss).toHaveBeenCalledTimes(1);
  });
});
