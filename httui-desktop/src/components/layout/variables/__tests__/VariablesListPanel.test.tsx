import { describe, it, expect, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { renderWithProviders, screen } from "@/test/render";
import { VariablesListPanel } from "@/components/layout/variables/VariablesListPanel";

describe("VariablesListPanel", () => {
  const defaultProps = {
    envColumnNames: ["local", "staging", "prod"] as const,
    searchValue: "",
    onSearchChange: vi.fn(),
  };

  it("renders the serif H1 + resolution hint + buttons", () => {
    renderWithProviders(<VariablesListPanel {...defaultProps} />);
    expect(screen.getByText("Variables")).toBeInTheDocument();
    expect(screen.getByTestId("variables-resolution-hint").textContent).toMatch(
      /block/,
    );
    expect(screen.getByTestId("variables-import-dotenv")).toBeInTheDocument();
    expect(screen.getByTestId("variables-create-new")).toBeInTheDocument();
  });

  it("disables Import / Nova when handlers are omitted", () => {
    renderWithProviders(<VariablesListPanel {...defaultProps} />);
    expect(
      (screen.getByTestId("variables-import-dotenv") as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    expect(
      (screen.getByTestId("variables-create-new") as HTMLButtonElement)
        .disabled,
    ).toBe(true);
  });

  it("renders the 3 env headers and the USES column", () => {
    renderWithProviders(<VariablesListPanel {...defaultProps} />);
    expect(
      screen.getByTestId("variables-env-header-local"),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId("variables-env-header-staging"),
    ).toBeInTheDocument();
    expect(screen.getByTestId("variables-env-header-prod")).toBeInTheDocument();
    expect(screen.getByTestId("variables-table-headers").textContent).toMatch(
      /USES/,
    );
  });

  it("pads the env header row when fewer than 3 envs are supplied", () => {
    renderWithProviders(
      <VariablesListPanel
        envColumnNames={["only-one"]}
        searchValue=""
        onSearchChange={vi.fn()}
      />,
    );
    expect(
      screen.getByTestId("variables-env-header-only-one"),
    ).toBeInTheDocument();
    const headers = screen.getByTestId("variables-table-headers");
    // Two placeholder em-dashes for the missing envs.
    expect(headers.textContent?.match(/—/g)?.length).toBeGreaterThanOrEqual(2);
  });

  it("only renders the first 3 envs when more are supplied", () => {
    renderWithProviders(
      <VariablesListPanel
        envColumnNames={["a", "b", "c", "d"]}
        searchValue=""
        onSearchChange={vi.fn()}
      />,
    );
    expect(screen.getByTestId("variables-env-header-a")).toBeInTheDocument();
    expect(screen.getByTestId("variables-env-header-b")).toBeInTheDocument();
    expect(screen.getByTestId("variables-env-header-c")).toBeInTheDocument();
    expect(
      screen.queryByTestId("variables-env-header-d"),
    ).not.toBeInTheDocument();
  });

  it("renders the active env pill when supplied", () => {
    renderWithProviders(
      <VariablesListPanel {...defaultProps} activeEnvName="staging" />,
    );
    expect(screen.getByTestId("variables-active-env-pill").textContent).toMatch(
      /staging/,
    );
  });

  it("typing in search dispatches onSearchChange", async () => {
    const onSearchChange = vi.fn();
    renderWithProviders(
      <VariablesListPanel
        envColumnNames={["local"]}
        searchValue=""
        onSearchChange={onSearchChange}
      />,
    );
    await userEvent.setup().type(screen.getByTestId("variables-search"), "x");
    expect(onSearchChange).toHaveBeenCalledWith("x");
  });

  it("clicking Import / Nova fires the handlers when supplied", async () => {
    const onImportDotenv = vi.fn();
    const onCreateNew = vi.fn();
    renderWithProviders(
      <VariablesListPanel
        {...defaultProps}
        onImportDotenv={onImportDotenv}
        onCreateNew={onCreateNew}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("variables-import-dotenv"));
    await user.click(screen.getByTestId("variables-create-new"));
    expect(onImportDotenv).toHaveBeenCalledTimes(1);
    expect(onCreateNew).toHaveBeenCalledTimes(1);
  });

  it("renders the empty hint when no rows slot is given", () => {
    renderWithProviders(<VariablesListPanel {...defaultProps} />);
    expect(screen.getByTestId("variables-empty-hint")).toBeInTheDocument();
  });

  it("renders the rowsSlot when supplied (no empty hint)", () => {
    renderWithProviders(
      <VariablesListPanel
        {...defaultProps}
        rowsSlot={<div data-testid="row-stub" />}
      />,
    );
    expect(screen.getByTestId("row-stub")).toBeInTheDocument();
    expect(
      screen.queryByTestId("variables-empty-hint"),
    ).not.toBeInTheDocument();
  });

  it("renders the footer keymap hint", () => {
    renderWithProviders(<VariablesListPanel {...defaultProps} />);
    expect(screen.getByTestId("variables-footer-hint").textContent).toMatch(
      /⌘⇧V new/,
    );
  });
});
