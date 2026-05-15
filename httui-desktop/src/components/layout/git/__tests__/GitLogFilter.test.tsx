import { describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { GitLogFilter } from "@/components/layout/git/GitLogFilter";
import type { LogFilterState } from "@/components/layout/git/git-log-filter";
import { renderWithProviders, screen } from "@/test/render";

function state(over: Partial<LogFilterState> = {}): LogFilterState {
  return { mode: "author", query: "", ...over };
}

describe("GitLogFilter", () => {
  it("renders a controlled input with the right placeholder per mode", () => {
    const { rerender } = renderWithProviders(
      <GitLogFilter state={state()} onChange={() => {}} />,
    );
    const input = screen.getByTestId(
      "git-log-filter-input",
    ) as HTMLInputElement;
    expect(input.placeholder).toBe("Filter by author…");

    rerender(<GitLogFilter state={state({ mode: "path" })} onChange={() => {}} />);
    expect(
      (screen.getByTestId("git-log-filter-input") as HTMLInputElement)
        .placeholder,
    ).toBe("Filter by path…");
  });

  it("encodes mode via data-mode + per-button data-active", () => {
    renderWithProviders(
      <GitLogFilter state={state({ mode: "author" })} onChange={() => {}} />,
    );
    expect(
      screen.getByTestId("git-log-filter").getAttribute("data-mode"),
    ).toBe("author");
    expect(
      screen
        .getByTestId("git-log-filter-mode-author")
        .getAttribute("data-active"),
    ).toBe("true");
    expect(
      screen
        .getByTestId("git-log-filter-mode-path")
        .getAttribute("data-active"),
    ).toBeNull();
  });

  it("fires onChange with the next state on input change", async () => {
    const onChange = vi.fn();
    renderWithProviders(
      <GitLogFilter state={state()} onChange={onChange} />,
    );
    await userEvent
      .setup()
      .type(screen.getByTestId("git-log-filter-input"), "j");
    expect(onChange).toHaveBeenCalled();
    expect(onChange.mock.calls[0]![0]).toEqual({ mode: "author", query: "j" });
  });

  it("fires onChange when toggling mode buttons", async () => {
    const onChange = vi.fn();
    renderWithProviders(
      <GitLogFilter state={state()} onChange={onChange} />,
    );
    await userEvent
      .setup()
      .click(screen.getByTestId("git-log-filter-mode-path"));
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange.mock.calls[0]![0].mode).toBe("path");
  });

  it("switches back to author mode via the Author button", async () => {
    const onChange = vi.fn();
    renderWithProviders(
      <GitLogFilter
        state={state({ mode: "path", query: "src" })}
        onChange={onChange}
      />,
    );
    await userEvent
      .setup()
      .click(screen.getByTestId("git-log-filter-mode-author"));
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange.mock.calls[0]![0]).toEqual({
      mode: "author",
      query: "src",
    });
  });

  it("hides clear button when query is empty", () => {
    renderWithProviders(<GitLogFilter state={state()} onChange={() => {}} />);
    expect(
      screen.queryByTestId("git-log-filter-clear"),
    ).not.toBeInTheDocument();
  });

  it("clears the query when clear is clicked", async () => {
    const onChange = vi.fn();
    renderWithProviders(
      <GitLogFilter state={state({ query: "alice" })} onChange={onChange} />,
    );
    await userEvent
      .setup()
      .click(screen.getByTestId("git-log-filter-clear"));
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange.mock.calls[0]![0].query).toBe("");
  });
});
