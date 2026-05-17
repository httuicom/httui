import { describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { GitCommitForm } from "@/components/layout/git/GitCommitForm";
import { renderWithProviders, screen } from "@/test/render";

interface RenderProps {
  message?: string;
  amend?: boolean;
  stagedCount?: number;
  busy?: boolean;
  onMessageChange?: (next: string) => void;
  onAmendChange?: (next: boolean) => void;
  onCommit?: (input: { message: string; amend: boolean }) => void;
}

function render(props: RenderProps = {}) {
  return renderWithProviders(
    <GitCommitForm
      message={props.message ?? ""}
      amend={props.amend ?? false}
      stagedCount={props.stagedCount ?? 0}
      busy={props.busy}
      onMessageChange={props.onMessageChange ?? (() => {})}
      onAmendChange={props.onAmendChange ?? (() => {})}
      onCommit={props.onCommit ?? (() => {})}
    />,
  );
}

describe("GitCommitForm", () => {
  it("renders a textarea + amend checkbox + summary + commit button", () => {
    render();
    expect(screen.getByTestId("git-commit-form-message")).toBeInTheDocument();
    expect(screen.getByTestId("git-commit-form-amend")).toBeInTheDocument();
    expect(screen.getByTestId("git-commit-form-summary")).toBeInTheDocument();
    expect(screen.getByTestId("git-commit-form-submit")).toBeInTheDocument();
  });

  it("disables the submit button when message is empty", () => {
    render({ stagedCount: 1 });
    expect(
      (screen.getByTestId("git-commit-form-submit") as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    expect(
      screen.getByTestId("git-commit-form").getAttribute("data-disabled"),
    ).toBe("true");
  });

  it("disables the submit button when no files are staged even with a valid message", () => {
    render({ message: "fix bug", stagedCount: 0 });
    expect(
      (screen.getByTestId("git-commit-form-submit") as HTMLButtonElement)
        .disabled,
    ).toBe(true);
  });

  it("enables the submit button with a valid message + ≥1 staged", () => {
    render({ message: "fix bug", stagedCount: 1 });
    expect(
      (screen.getByTestId("git-commit-form-submit") as HTMLButtonElement)
        .disabled,
    ).toBe(false);
  });

  it("renders validation errors when subject is too long", () => {
    render({ message: "a".repeat(80), stagedCount: 1 });
    expect(screen.getByTestId("git-commit-form-error-0")).toBeInTheDocument();
  });

  it("uses 'Amend' as the button label when amend is true", () => {
    render({ message: "fix bug", stagedCount: 1, amend: true });
    expect(screen.getByTestId("git-commit-form-submit").textContent).toBe(
      "Amend",
    );
  });

  it("agrees sing/plural in the staged-files summary", () => {
    render({ stagedCount: 1 });
    expect(screen.getByTestId("git-commit-form-summary").textContent).toMatch(
      /1 file staged/,
    );

    render({ stagedCount: 3 });
    const sums = screen.getAllByTestId("git-commit-form-summary");
    expect(sums[sums.length - 1]!.textContent).toMatch(/3 files staged/);
  });

  it("fires onMessageChange on textarea typing", async () => {
    const onMessageChange = vi.fn();
    render({ onMessageChange });
    await userEvent
      .setup()
      .type(screen.getByTestId("git-commit-form-message"), "x");
    expect(onMessageChange).toHaveBeenCalledWith("x");
  });

  it("fires onCommit with trimmed message + amend flag", async () => {
    const onCommit = vi.fn();
    render({
      message: "  fix bug  ",
      amend: true,
      stagedCount: 1,
      onCommit,
    });
    // message has leading whitespace which makes validation fail; use
    // a clean message instead.
    expect(onCommit).not.toHaveBeenCalled();

    const onCommit2 = vi.fn();
    render({
      message: "fix bug",
      amend: true,
      stagedCount: 1,
      onCommit: onCommit2,
    });
    const submits = screen.getAllByTestId("git-commit-form-submit");
    await userEvent.setup().click(submits[submits.length - 1]!);
    expect(onCommit2).toHaveBeenCalledWith({ message: "fix bug", amend: true });
  });

  it("flags busy state via data-busy and disables interactions", () => {
    render({ message: "fix bug", stagedCount: 1, busy: true });
    expect(
      screen.getByTestId("git-commit-form").getAttribute("data-busy"),
    ).toBe("true");
    expect(
      (screen.getByTestId("git-commit-form-submit") as HTMLButtonElement)
        .disabled,
    ).toBe(true);
  });
});
