import { describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { GitConflictBanner } from "@/components/layout/git/GitConflictBanner";
import { renderWithProviders, screen } from "@/test/render";

describe("GitConflictBanner", () => {
  it("renders nothing when conflicts array is empty", () => {
    const { container } = renderWithProviders(
      <GitConflictBanner conflicts={[]} />,
    );
    expect(
      container.querySelector('[data-testid="git-conflict-banner"]'),
    ).toBeNull();
  });

  it("renders with sing/plural agreement on the count", () => {
    renderWithProviders(<GitConflictBanner conflicts={["a.md"]} />);
    expect(screen.getByTestId("git-conflict-banner").textContent).toMatch(
      /1 conflict to resolve/,
    );
  });

  it("agrees plural for >1 conflicts", () => {
    renderWithProviders(<GitConflictBanner conflicts={["a.md", "b.md"]} />);
    expect(screen.getByTestId("git-conflict-banner").textContent).toMatch(
      /2 conflicts to resolve/,
    );
  });

  it("renders one row per conflict", () => {
    renderWithProviders(
      <GitConflictBanner conflicts={["a.md", "nested/b.md"]} />,
    );
    expect(screen.getByTestId("git-conflict-row-a.md")).toBeInTheDocument();
    expect(
      screen.getByTestId("git-conflict-row-nested/b.md"),
    ).toBeInTheDocument();
  });

  it("encodes count via data-count", () => {
    renderWithProviders(<GitConflictBanner conflicts={["a", "b", "c"]} />);
    expect(
      screen.getByTestId("git-conflict-banner").getAttribute("data-count"),
    ).toBe("3");
  });

  it("only renders the action buttons whose handlers are provided", () => {
    renderWithProviders(
      <GitConflictBanner conflicts={["a.md"]} onOpenDiff={() => {}} />,
    );
    expect(
      screen.getByTestId("git-conflict-row-a.md-resolve"),
    ).toBeInTheDocument();
    expect(
      screen.queryByTestId("git-conflict-row-a.md-accept-yours"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByTestId("git-conflict-row-a.md-accept-theirs"),
    ).not.toBeInTheDocument();
  });

  it("fires onOpenDiff with the path on Resolve click", async () => {
    const onOpenDiff = vi.fn();
    renderWithProviders(
      <GitConflictBanner conflicts={["a.md"]} onOpenDiff={onOpenDiff} />,
    );
    await userEvent
      .setup()
      .click(screen.getByTestId("git-conflict-row-a.md-resolve"));
    expect(onOpenDiff).toHaveBeenCalledWith("a.md");
  });

  it("fires onAcceptYours on Accept-yours click", async () => {
    const onAcceptYours = vi.fn();
    renderWithProviders(
      <GitConflictBanner conflicts={["a.md"]} onAcceptYours={onAcceptYours} />,
    );
    await userEvent
      .setup()
      .click(screen.getByTestId("git-conflict-row-a.md-accept-yours"));
    expect(onAcceptYours).toHaveBeenCalledWith("a.md");
  });

  it("fires onAcceptTheirs on Accept-theirs click", async () => {
    const onAcceptTheirs = vi.fn();
    renderWithProviders(
      <GitConflictBanner
        conflicts={["a.md"]}
        onAcceptTheirs={onAcceptTheirs}
      />,
    );
    await userEvent
      .setup()
      .click(screen.getByTestId("git-conflict-row-a.md-accept-theirs"));
    expect(onAcceptTheirs).toHaveBeenCalledWith("a.md");
  });

  it("disables every button while busy", () => {
    renderWithProviders(
      <GitConflictBanner
        conflicts={["a.md"]}
        busy
        onOpenDiff={() => {}}
        onAcceptYours={() => {}}
        onAcceptTheirs={() => {}}
      />,
    );
    expect(
      (screen.getByTestId("git-conflict-row-a.md-resolve") as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    expect(
      (
        screen.getByTestId(
          "git-conflict-row-a.md-accept-yours",
        ) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    expect(
      (
        screen.getByTestId(
          "git-conflict-row-a.md-accept-theirs",
        ) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    expect(
      screen.getByTestId("git-conflict-banner").getAttribute("data-busy"),
    ).toBe("true");
  });
});
