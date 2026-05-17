import { describe, it, expect, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { renderWithProviders, screen } from "@/test/render";
import { NewConnectionStringTab } from "@/components/layout/connections/NewConnectionStringTab";

describe("NewConnectionStringTab", () => {
  it("disables Apply when the textarea is empty", () => {
    renderWithProviders(<NewConnectionStringTab onApply={vi.fn()} />);
    expect(
      (screen.getByTestId("new-connection-string-apply") as HTMLButtonElement)
        .disabled,
    ).toBe(true);
  });

  it("renders the initial text", () => {
    renderWithProviders(
      <NewConnectionStringTab
        initial="postgres://localhost/x"
        onApply={vi.fn()}
      />,
    );
    expect(
      (screen.getByTestId("new-connection-string-input") as HTMLTextAreaElement)
        .value,
    ).toBe("postgres://localhost/x");
  });

  it("dispatches onApply with parsed kind/value/ssl on Apply click", async () => {
    const onApply = vi.fn();
    renderWithProviders(<NewConnectionStringTab onApply={onApply} />);
    const user = userEvent.setup();
    await user.type(
      screen.getByTestId("new-connection-string-input"),
      "postgres://u:p@h:5432/db?sslmode=require",
    );
    await user.click(screen.getByTestId("new-connection-string-apply"));
    expect(onApply).toHaveBeenCalledTimes(1);
    expect(onApply).toHaveBeenCalledWith({
      kind: "postgres",
      value: expect.objectContaining({ host: "h", database: "db" }),
      ssl: expect.objectContaining({ mode: "require" }),
    });
    expect(
      screen.getByTestId("new-connection-string-success"),
    ).toBeInTheDocument();
  });

  it("surfaces a parse error and does NOT call onApply on bad input", async () => {
    const onApply = vi.fn();
    renderWithProviders(<NewConnectionStringTab onApply={onApply} />);
    const user = userEvent.setup();
    await user.type(
      screen.getByTestId("new-connection-string-input"),
      "not-a-url",
    );
    await user.click(screen.getByTestId("new-connection-string-apply"));
    expect(onApply).not.toHaveBeenCalled();
    expect(
      screen.getByTestId("new-connection-string-error"),
    ).toBeInTheDocument();
  });

  it("re-enables apply once user types non-empty content", async () => {
    renderWithProviders(<NewConnectionStringTab onApply={vi.fn()} />);
    const user = userEvent.setup();
    await user.type(screen.getByTestId("new-connection-string-input"), "x");
    expect(
      (screen.getByTestId("new-connection-string-apply") as HTMLButtonElement)
        .disabled,
    ).toBe(false);
  });
});
