import { describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { VariableValueRow } from "@/components/layout/variables/VariableValueRow";
import { renderWithProviders, screen } from "@/test/render";

describe("VariableValueRow (read-only + reveal)", () => {
  it("renders the env label and the cleartext value for a non-secret row", () => {
    renderWithProviders(
      <VariableValueRow
        env="local"
        value="http://localhost"
        isSecret={false}
      />,
    );
    expect(
      screen.getByTestId("variable-value-row-local-env-label").textContent,
    ).toBe("local");
    expect(
      screen.getByTestId("variable-value-row-local-display").textContent,
    ).toBe("http://localhost");
    expect(
      screen.queryByTestId("variable-value-row-local-show"),
    ).not.toBeInTheDocument();
  });

  it("renders an em-dash when the value is undefined and the row is not secret", () => {
    renderWithProviders(
      <VariableValueRow env="prod" value={undefined} isSecret={false} />,
    );
    expect(
      screen.getByTestId("variable-value-row-prod-display").textContent,
    ).toBe("—");
  });

  it("masks a secret value with bullets and shows a Show button", () => {
    renderWithProviders(
      <VariableValueRow env="staging" value={undefined} isSecret={true} />,
    );
    expect(
      screen.getByTestId("variable-value-row-staging-display").textContent,
    ).toBe("••••••••");
    expect(
      screen.getByTestId("variable-value-row-staging-show"),
    ).toBeInTheDocument();
  });

  it("disables Show when fetchSecret is not provided", () => {
    renderWithProviders(
      <VariableValueRow env="staging" value={undefined} isSecret={true} />,
    );
    expect(
      (
        screen.getByTestId(
          "variable-value-row-staging-show",
        ) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
  });

  it("reveals the cleartext via fetchSecret and toggles back on Hide", async () => {
    const fetchSecret = vi.fn(async (env: string) => `cleartext-for-${env}`);
    renderWithProviders(
      <VariableValueRow
        env="staging"
        value={undefined}
        isSecret={true}
        fetchSecret={fetchSecret}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("variable-value-row-staging-show"));
    expect(fetchSecret).toHaveBeenCalledWith("staging");
    expect(
      screen.getByTestId("variable-value-row-staging-display").textContent,
    ).toBe("cleartext-for-staging");
    expect(
      screen.getByTestId("variable-value-row-staging-hide"),
    ).toBeInTheDocument();

    await user.click(screen.getByTestId("variable-value-row-staging-hide"));
    expect(
      screen.getByTestId("variable-value-row-staging-display").textContent,
    ).toBe("••••••••");
    expect(
      screen.getByTestId("variable-value-row-staging-show"),
    ).toBeInTheDocument();
  });

  it("renders an inline error when fetchSecret rejects", async () => {
    const fetchSecret = vi.fn(async () => {
      throw new Error("keychain locked");
    });
    renderWithProviders(
      <VariableValueRow
        env="staging"
        value={undefined}
        isSecret={true}
        fetchSecret={fetchSecret}
      />,
    );
    await userEvent
      .setup()
      .click(screen.getByTestId("variable-value-row-staging-show"));
    expect(
      screen.getByTestId("variable-value-row-staging-display").textContent,
    ).toMatch(/keychain locked/);
  });

  it("renders an empty hint when the revealed cleartext is the empty string", async () => {
    const fetchSecret = vi.fn(async () => "");
    renderWithProviders(
      <VariableValueRow
        env="staging"
        value={undefined}
        isSecret={true}
        fetchSecret={fetchSecret}
      />,
    );
    await userEvent
      .setup()
      .click(screen.getByTestId("variable-value-row-staging-show"));
    expect(
      screen.getByTestId("variable-value-row-staging-display").textContent,
    ).toMatch(/empty/i);
  });

  it("normalizes a non-Error rejection to its string form", async () => {
    const fetchSecret = vi.fn(async () => {
      throw "raw-string-error";
    });
    renderWithProviders(
      <VariableValueRow
        env="staging"
        value={undefined}
        isSecret={true}
        fetchSecret={fetchSecret}
      />,
    );
    await userEvent
      .setup()
      .click(screen.getByTestId("variable-value-row-staging-show"));
    expect(
      screen.getByTestId("variable-value-row-staging-display").textContent,
    ).toMatch(/raw-string-error/);
  });

  it("returning undefined from fetchSecret renders the empty cleartext hint", async () => {
    const fetchSecret = vi.fn(async () => undefined);
    renderWithProviders(
      <VariableValueRow
        env="staging"
        value={undefined}
        isSecret={true}
        fetchSecret={fetchSecret}
      />,
    );
    await userEvent
      .setup()
      .click(screen.getByTestId("variable-value-row-staging-show"));
    expect(
      screen.getByTestId("variable-value-row-staging-display").textContent,
    ).toMatch(/empty/i);
  });
});

describe("VariableValueRow (edit mode)", () => {
  it("hides the Edit button when no onCommit handler is supplied", () => {
    renderWithProviders(
      <VariableValueRow env="local" value="x" isSecret={false} />,
    );
    expect(
      screen.queryByTestId("variable-value-row-local-edit"),
    ).not.toBeInTheDocument();
  });

  it("hides the Edit button on a masked secret row", () => {
    renderWithProviders(
      <VariableValueRow
        env="staging"
        value={undefined}
        isSecret={true}
        onCommit={() => {}}
      />,
    );
    expect(
      screen.queryByTestId("variable-value-row-staging-edit"),
    ).not.toBeInTheDocument();
  });

  it("enters edit mode for a non-secret row pre-filled with the current value", async () => {
    const onCommit = vi.fn();
    renderWithProviders(
      <VariableValueRow
        env="local"
        value="http://localhost"
        isSecret={false}
        onCommit={onCommit}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("variable-value-row-local-edit"));
    const input = screen.getByTestId(
      "variable-value-row-local-input",
    ) as HTMLInputElement;
    expect(input.value).toBe("http://localhost");
    expect(
      screen.getByTestId("variable-value-row-local").getAttribute("data-mode"),
    ).toBe("commit");
  });

  it("commits the draft on Save and returns to view mode", async () => {
    const onCommit = vi.fn();
    renderWithProviders(
      <VariableValueRow
        env="local"
        value="old"
        isSecret={false}
        onCommit={onCommit}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("variable-value-row-local-edit"));
    const input = screen.getByTestId(
      "variable-value-row-local-input",
    ) as HTMLInputElement;
    await user.clear(input);
    await user.type(input, "new-value");
    await user.click(screen.getByTestId("variable-value-row-local-save"));
    expect(onCommit).toHaveBeenCalledWith("local", "new-value");
    expect(
      screen.getByTestId("variable-value-row-local").getAttribute("data-mode"),
    ).toBe("view");
  });

  it("discards the draft on Cancel without calling onCommit", async () => {
    const onCommit = vi.fn();
    renderWithProviders(
      <VariableValueRow
        env="local"
        value="old"
        isSecret={false}
        onCommit={onCommit}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("variable-value-row-local-edit"));
    const input = screen.getByTestId(
      "variable-value-row-local-input",
    ) as HTMLInputElement;
    await user.clear(input);
    await user.type(input, "discarded");
    await user.click(screen.getByTestId("variable-value-row-local-cancel"));
    expect(onCommit).not.toHaveBeenCalled();
    expect(
      screen.getByTestId("variable-value-row-local-display").textContent,
    ).toBe("old");
  });

  it("commits on Enter and cancels on Escape", async () => {
    const onCommit = vi.fn();
    renderWithProviders(
      <VariableValueRow
        env="local"
        value="initial"
        isSecret={false}
        onCommit={onCommit}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("variable-value-row-local-edit"));
    const input = screen.getByTestId("variable-value-row-local-input");
    await user.type(input, "{Enter}");
    expect(onCommit).toHaveBeenCalledWith("local", "initial");

    await user.click(screen.getByTestId("variable-value-row-local-edit"));
    const input2 = screen.getByTestId("variable-value-row-local-input");
    await user.type(input2, "{Escape}");
    expect(onCommit).toHaveBeenCalledTimes(1);
  });

  it("treats undefined value as the empty draft when entering edit", async () => {
    const onCommit = vi.fn();
    renderWithProviders(
      <VariableValueRow
        env="prod"
        value={undefined}
        isSecret={false}
        onCommit={onCommit}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("variable-value-row-prod-edit"));
    expect(
      (screen.getByTestId("variable-value-row-prod-input") as HTMLInputElement)
        .value,
    ).toBe("");
  });

  it("renders the override value and TEMPORARY chip in override mode", () => {
    renderWithProviders(
      <VariableValueRow
        env="local"
        value="from-toml"
        isSecret={false}
        override="from-session"
      />,
    );
    expect(
      screen.getByTestId("variable-value-row-local-display").textContent,
    ).toBe("from-session");
    expect(screen.getByTestId("temporary-chip")).toBeInTheDocument();
    expect(
      screen.getByTestId("variable-value-row-local").getAttribute("data-mode"),
    ).toBe("override");
  });

  it("hides Show/Hide and Edit buttons while overridden", () => {
    renderWithProviders(
      <VariableValueRow
        env="staging"
        value="x"
        isSecret={true}
        override="session-value"
        onCommit={() => {}}
        fetchSecret={async () => "real"}
      />,
    );
    expect(
      screen.queryByTestId("variable-value-row-staging-show"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByTestId("variable-value-row-staging-edit"),
    ).not.toBeInTheDocument();
  });

  it("clears the override when the chip is clicked", async () => {
    const onClearOverride = vi.fn();
    renderWithProviders(
      <VariableValueRow
        env="local"
        value="from-toml"
        isSecret={false}
        override="from-session"
        onClearOverride={onClearOverride}
      />,
    );
    await userEvent.setup().click(screen.getByTestId("temporary-chip"));
    expect(onClearOverride).toHaveBeenCalledTimes(1);
  });

  it("renders override mode for an empty-string override (treated as active)", () => {
    renderWithProviders(
      <VariableValueRow
        env="local"
        value="from-toml"
        isSecret={false}
        override=""
      />,
    );
    expect(
      screen.getByTestId("variable-value-row-local").getAttribute("data-mode"),
    ).toBe("override");
  });

  it("allows edit on a revealed secret and uses the revealed cleartext as draft", async () => {
    const fetchSecret = vi.fn(async () => "real-token");
    const onCommit = vi.fn();
    renderWithProviders(
      <VariableValueRow
        env="staging"
        value={undefined}
        isSecret={true}
        fetchSecret={fetchSecret}
        onCommit={onCommit}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("variable-value-row-staging-show"));
    expect(
      screen.getByTestId("variable-value-row-staging-edit"),
    ).toBeInTheDocument();
    await user.click(screen.getByTestId("variable-value-row-staging-edit"));
    const input = screen.getByTestId(
      "variable-value-row-staging-input",
    ) as HTMLInputElement;
    expect(input.value).toBe("real-token");
    await user.clear(input);
    await user.type(input, "rotated-token");
    await user.click(screen.getByTestId("variable-value-row-staging-save"));
    expect(onCommit).toHaveBeenCalledWith("staging", "rotated-token");
    expect(
      screen.getByTestId("variable-value-row-staging-display").textContent,
    ).toBe("rotated-token");
  });
});
