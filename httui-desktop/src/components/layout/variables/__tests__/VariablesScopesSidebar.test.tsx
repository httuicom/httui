import { describe, it, expect, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { renderWithProviders, screen } from "@/test/render";
import { VariablesScopesSidebar } from "@/components/layout/variables/VariablesScopesSidebar";
import {
  VARIABLE_HELPERS,
  VARIABLE_SCOPES,
} from "@/components/layout/variables/variable-scopes";

describe("VariablesScopesSidebar", () => {
  it("renders the 5 scopes with default count of 0", () => {
    renderWithProviders(
      <VariablesScopesSidebar selectedScope="all" onSelectScope={vi.fn()} />,
    );
    for (const scope of VARIABLE_SCOPES) {
      expect(
        screen.getByTestId(`variables-scope-${scope}`),
      ).toBeInTheDocument();
      expect(
        screen.getByTestId(`variables-scope-${scope}-count`).textContent,
      ).toBe("0");
    }
  });

  it("marks the selected scope active and shows the supplied counts", () => {
    renderWithProviders(
      <VariablesScopesSidebar
        selectedScope="secret"
        onSelectScope={vi.fn()}
        countsByScope={{ all: 8, workspace: 3, secret: 3 }}
      />,
    );
    expect(
      screen
        .getByTestId("variables-scope-secret")
        .getAttribute("data-selected"),
    ).toBe("true");
    expect(
      screen
        .getByTestId("variables-scope-all")
        .getAttribute("data-selected"),
    ).toBe("false");
    expect(screen.getByTestId("variables-scope-all-count").textContent).toBe(
      "8",
    );
    expect(
      screen.getByTestId("variables-scope-workspace-count").textContent,
    ).toBe("3");
  });

  it("clicking a scope dispatches onSelectScope", async () => {
    const onSelectScope = vi.fn();
    renderWithProviders(
      <VariablesScopesSidebar
        selectedScope="all"
        onSelectScope={onSelectScope}
      />,
    );
    await userEvent
      .setup()
      .click(screen.getByTestId("variables-scope-workspace"));
    expect(onSelectScope).toHaveBeenCalledWith("workspace");
  });

  it("Enter / Space on a focused row dispatches onSelectScope", async () => {
    const onSelectScope = vi.fn();
    renderWithProviders(
      <VariablesScopesSidebar
        selectedScope="all"
        onSelectScope={onSelectScope}
      />,
    );
    const captured = screen.getByTestId("variables-scope-captured");
    captured.focus();
    await userEvent.setup().keyboard("{Enter}");
    expect(onSelectScope).toHaveBeenCalledWith("captured");
    await userEvent.setup().keyboard(" ");
    expect(onSelectScope).toHaveBeenCalledTimes(2);
  });

  it("renders all 4 helpers and the secrets hint", () => {
    renderWithProviders(
      <VariablesScopesSidebar selectedScope="all" onSelectScope={vi.fn()} />,
    );
    for (const helper of VARIABLE_HELPERS) {
      expect(
        screen.getByTestId(`variables-helper-${helper.syntax}`),
      ).toBeInTheDocument();
    }
    expect(screen.getByTestId("variables-secrets-hint")).toBeInTheDocument();
    expect(screen.getByText(/Local secrets/)).toBeInTheDocument();
  });
});
