import { describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { EnvironmentCard } from "@/components/layout/environments/EnvironmentCard";
import type { EnvironmentSummary } from "@/components/layout/environments/envs-meta";
import { renderWithProviders, screen } from "@/test/render";

function env(over: Partial<EnvironmentSummary> = {}): EnvironmentSummary {
  return {
    name: "local",
    filename: "local.toml",
    varCount: 5,
    connectionsUsedCount: 0,
    isActive: false,
    isPersonal: false,
    isTemporary: false,
    ...over,
  };
}

describe("EnvironmentCard", () => {
  it("renders the env name + var count + 'all conns' default", () => {
    renderWithProviders(<EnvironmentCard env={env()} />);
    expect(
      screen.getByTestId("environment-card-local.toml-name").textContent,
    ).toBe("local");
    expect(
      screen.getByTestId("environment-card-local.toml-vars").textContent,
    ).toBe("5 vars");
    expect(
      screen.getByTestId("environment-card-local.toml-conns").textContent,
    ).toBe("all conns");
  });

  it("uses singular 'var' / 'conn' when the count is 1", () => {
    renderWithProviders(
      <EnvironmentCard env={env({ varCount: 1, connectionsUsedCount: 1 })} />,
    );
    expect(
      screen.getByTestId("environment-card-local.toml-vars").textContent,
    ).toBe("1 var");
    expect(
      screen.getByTestId("environment-card-local.toml-conns").textContent,
    ).toBe("1 conn");
  });

  it("renders the ACTIVE pill when isActive is true", () => {
    renderWithProviders(<EnvironmentCard env={env({ isActive: true })} />);
    expect(
      screen.getByTestId("environment-card-local.toml-active-pill"),
    ).toBeInTheDocument();
    expect(
      screen
        .getByTestId("environment-card-local.toml")
        .getAttribute("data-active"),
    ).toBe("true");
  });

  it("renders the personal chip + data attribute when isPersonal is true", () => {
    renderWithProviders(
      <EnvironmentCard
        env={env({ filename: "local.local.toml", isPersonal: true })}
      />,
    );
    expect(
      screen.getByTestId("environment-card-local.local.toml-personal-chip"),
    ).toBeInTheDocument();
    expect(
      screen
        .getByTestId("environment-card-local.local.toml")
        .getAttribute("data-personal"),
    ).toBe("true");
  });

  it("renders the temporary chip when isTemporary is true", () => {
    renderWithProviders(<EnvironmentCard env={env({ isTemporary: true })} />);
    expect(
      screen.getByTestId("environment-card-local.toml-temporary-chip"),
    ).toBeInTheDocument();
  });

  it("renders the description when present", () => {
    renderWithProviders(
      <EnvironmentCard env={env({ description: "for staging only" })} />,
    );
    expect(
      screen.getByTestId("environment-card-local.toml-description").textContent,
    ).toBe("for staging only");
  });

  it("is a non-interactive div when onActivate is omitted", () => {
    renderWithProviders(<EnvironmentCard env={env()} />);
    expect(screen.getByTestId("environment-card-local.toml").tagName).toBe(
      "DIV",
    );
  });

  it("renders an inner activate button that fires onActivate(filename)", async () => {
    const onActivate = vi.fn();
    renderWithProviders(
      <EnvironmentCard env={env()} onActivate={onActivate} />,
    );
    const card = screen.getByTestId("environment-card-local.toml");
    const activateBtn = card.querySelector("button");
    expect(activateBtn).not.toBeNull();
    await userEvent.setup().click(activateBtn!);
    expect(onActivate).toHaveBeenCalledWith("local.toml");
  });

  it("renders neither chip when isPersonal and isTemporary are both false", () => {
    renderWithProviders(<EnvironmentCard env={env()} />);
    expect(
      screen.queryByTestId("environment-card-local.toml-personal-chip"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByTestId("environment-card-local.toml-temporary-chip"),
    ).not.toBeInTheDocument();
  });
});
