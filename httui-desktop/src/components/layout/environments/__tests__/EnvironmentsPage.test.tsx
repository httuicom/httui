import { describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { EnvironmentsPage } from "@/components/layout/environments/EnvironmentsPage";
import type { EnvironmentSummary } from "@/components/layout/environments/envs-meta";
import { renderWithProviders, screen } from "@/test/render";

function env(over: Partial<EnvironmentSummary>): EnvironmentSummary {
  return {
    name: "local",
    filename: "local.toml",
    varCount: 0,
    connectionsUsedCount: 0,
    isActive: false,
    isPersonal: false,
    isTemporary: false,
    ...over,
  };
}

describe("EnvironmentsPage", () => {
  it("renders the page header and subtitle", () => {
    renderWithProviders(<EnvironmentsPage envs={[]} />);
    expect(screen.getByTestId("environments-page")).toBeInTheDocument();
    expect(
      screen.getByTestId("environments-page-subtitle").textContent,
    ).toMatch(/local\.toml/);
  });

  it("renders the empty hint when envs is empty", () => {
    renderWithProviders(<EnvironmentsPage envs={[]} />);
    expect(screen.getByTestId("environments-empty-hint")).toBeInTheDocument();
    expect(screen.queryByTestId("environments-grid")).not.toBeInTheDocument();
  });

  it("renders one card per env in alphabetical order regardless of isActive", () => {
    renderWithProviders(
      <EnvironmentsPage
        envs={[
          env({ name: "zeta", filename: "zeta.toml" }),
          env({ name: "alpha", filename: "alpha.toml" }),
          env({ name: "beta", filename: "beta.toml", isActive: true }),
        ]}
      />,
    );
    const cards = [
      ...document.querySelectorAll("[data-testid^=environment-card-]"),
    ]
      .map((el) => el.getAttribute("data-testid"))
      .filter(
        (id): id is string => !!id && /^environment-card-[^-]+$/.test(id),
      );
    expect(cards).toEqual([
      "environment-card-alpha.toml",
      "environment-card-beta.toml",
      "environment-card-zeta.toml",
    ]);
  });

  it("disables the create button when no onCreateNew handler is supplied", () => {
    renderWithProviders(<EnvironmentsPage envs={[]} />);
    expect(
      (screen.getByTestId("environments-create-new") as HTMLButtonElement)
        .disabled,
    ).toBe(true);
  });

  it("fires onCreateNew when the button is clicked", async () => {
    const onCreateNew = vi.fn();
    renderWithProviders(
      <EnvironmentsPage envs={[]} onCreateNew={onCreateNew} />,
    );
    await userEvent
      .setup()
      .click(screen.getByTestId("environments-create-new"));
    expect(onCreateNew).toHaveBeenCalledTimes(1);
  });

  it("forwards onActivate down to each card", async () => {
    const onActivate = vi.fn();
    renderWithProviders(
      <EnvironmentsPage
        envs={[env({ name: "staging", filename: "staging.toml" })]}
        onActivate={onActivate}
      />,
    );
    const card = screen.getByTestId("environment-card-staging.toml");
    const activateBtn = card.querySelector("button");
    await userEvent.setup().click(activateBtn!);
    expect(onActivate).toHaveBeenCalledWith("staging.toml");
  });

  it("renders the inlineFormSlot above the grid when supplied", () => {
    renderWithProviders(
      <EnvironmentsPage
        envs={[]}
        inlineFormSlot={<div data-testid="form-stub" />}
      />,
    );
    expect(screen.getByTestId("form-stub")).toBeInTheDocument();
  });
});
