import { describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { VariableDetailContent } from "@/components/layout/variables/VariableDetailContent";
import type { VariableRow } from "@/components/layout/variables/variable-derive";
import { renderWithProviders, screen } from "@/test/render";

function row(over: Partial<VariableRow> = {}): VariableRow {
  return {
    key: "API_BASE",
    scope: "workspace",
    isSecret: false,
    values: {
      local: "http://localhost",
      staging: "https://stg.example",
      prod: "https://api.example",
    },
    usesCount: 3,
    ...over,
  };
}

describe("VariableDetailContent", () => {
  it("renders the header + one value row per env in display order", () => {
    renderWithProviders(
      <VariableDetailContent
        row={row()}
        envNames={["local", "staging", "prod"]}
      />,
    );
    expect(screen.getByTestId("variable-detail-header")).toBeInTheDocument();
    expect(screen.getByTestId("variable-value-row-local")).toBeInTheDocument();
    expect(
      screen.getByTestId("variable-value-row-staging"),
    ).toBeInTheDocument();
    expect(screen.getByTestId("variable-value-row-prod")).toBeInTheDocument();
  });

  it("renders the empty-envs hint when envNames is empty", () => {
    renderWithProviders(<VariableDetailContent row={row()} envNames={[]} />);
    expect(
      screen.getByTestId("variable-detail-empty-envs"),
    ).toBeInTheDocument();
  });

  it("forwards fetchSecret to each value row when isSecret is true", async () => {
    const fetchSecret = vi.fn(async (env: string) => `clear-${env}`);
    renderWithProviders(
      <VariableDetailContent
        row={row({ isSecret: true, scope: "personal", values: {} })}
        envNames={["staging"]}
        fetchSecret={fetchSecret}
      />,
    );
    expect(
      (
        screen.getByTestId(
          "variable-value-row-staging-show",
        ) as HTMLButtonElement
      ).disabled,
    ).toBe(false);
  });

  it("renders the uses placeholder text with count when usesCount > 0", () => {
    renderWithProviders(
      <VariableDetailContent
        row={row({ usesCount: 5 })}
        envNames={["local"]}
      />,
    );
    expect(
      screen.getByTestId("variable-detail-uses-placeholder").textContent,
    ).toMatch(/5 references/);
  });

  it("uses singular wording when usesCount === 1", () => {
    renderWithProviders(
      <VariableDetailContent
        row={row({ usesCount: 1 })}
        envNames={["local"]}
      />,
    );
    expect(
      screen.getByTestId("variable-detail-uses-placeholder").textContent,
    ).toMatch(/1 reference\b/);
  });

  it("renders the no-references variant when usesCount === 0", () => {
    renderWithProviders(
      <VariableDetailContent
        row={row({ usesCount: 0 })}
        envNames={["local"]}
      />,
    );
    expect(
      screen.getByTestId("variable-detail-uses-placeholder").textContent,
    ).toMatch(/No references/);
  });

  it("renders usedInBlocksSlot in place of the placeholder when supplied", () => {
    renderWithProviders(
      <VariableDetailContent
        row={row()}
        envNames={["local"]}
        usedInBlocksSlot={<div data-testid="custom-uses">CUSTOM</div>}
      />,
    );
    expect(screen.getByTestId("custom-uses")).toBeInTheDocument();
    expect(
      screen.queryByTestId("variable-detail-uses-placeholder"),
    ).not.toBeInTheDocument();
  });

  it("renders the is_secret toggle and forwards onToggleSecret + confirmDemote", async () => {
    const onToggleSecret = vi.fn();
    const confirmDemote = vi.fn(async () => true);
    renderWithProviders(
      <VariableDetailContent
        row={row({ isSecret: true, scope: "personal" })}
        envNames={["local"]}
        onToggleSecret={onToggleSecret}
        confirmDemote={confirmDemote}
      />,
    );
    expect(screen.getByTestId("variable-secret-toggle")).toBeInTheDocument();
    await userEvent
      .setup()
      .click(screen.getByTestId("variable-secret-toggle-switch"));
    expect(confirmDemote).toHaveBeenCalledTimes(1);
    expect(onToggleSecret).toHaveBeenCalledWith(false);
  });

  it("forwards onCommitValue down to each value row", async () => {
    const onCommitValue = vi.fn();
    renderWithProviders(
      <VariableDetailContent
        row={row({ values: { local: "x" } })}
        envNames={["local"]}
        onCommitValue={onCommitValue}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("variable-value-row-local-edit"));
    const input = screen.getByTestId(
      "variable-value-row-local-input",
    ) as HTMLInputElement;
    await user.clear(input);
    await user.type(input, "y");
    await user.click(screen.getByTestId("variable-value-row-local-save"));
    expect(onCommitValue).toHaveBeenCalledWith("local", "y");
  });

  it("uses row.values[env] for each value cell", () => {
    renderWithProviders(
      <VariableDetailContent
        row={row({ values: { local: "X", prod: undefined } })}
        envNames={["local", "staging", "prod"]}
      />,
    );
    expect(
      screen.getByTestId("variable-value-row-local-display").textContent,
    ).toBe("X");
    expect(
      screen.getByTestId("variable-value-row-staging-display").textContent,
    ).toBe("—");
    expect(
      screen.getByTestId("variable-value-row-prod-display").textContent,
    ).toBe("—");
  });
});
