import { describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { VariableListRow } from "@/components/layout/variables/VariableListRow";
import type { VariableRow } from "@/components/layout/variables/variable-derive";
import { renderWithProviders, screen } from "@/test/render";

function row(over: Partial<VariableRow> = {}): VariableRow {
  return {
    key: "API_BASE",
    scope: "workspace",
    isSecret: false,
    values: {
      local: "http://localhost:3000",
      staging: "https://stg.example",
      prod: "https://api.example",
    },
    usesCount: 4,
    ...over,
  };
}

describe("VariableListRow", () => {
  it("renders the key, scope glyph, and per-env values", () => {
    renderWithProviders(
      <VariableListRow
        row={row()}
        envColumnNames={["local", "staging", "prod"]}
      />,
    );
    expect(screen.getByTestId("variables-row-API_BASE-key").textContent).toBe(
      "API_BASE",
    );
    expect(
      screen.getByTestId("variables-row-API_BASE-value-local").textContent,
    ).toBe("http://localhost:3000");
    expect(
      screen.getByTestId("variables-row-API_BASE-value-staging").textContent,
    ).toBe("https://stg.example");
    expect(screen.getByTestId("variables-row-API_BASE-uses").textContent).toBe(
      "4",
    );
  });

  it("renders em-dash for undefined env values", () => {
    renderWithProviders(
      <VariableListRow
        row={row({ values: { local: "x", prod: undefined } })}
        envColumnNames={["local", "staging", "prod"]}
      />,
    );
    expect(
      screen.getByTestId("variables-row-API_BASE-value-staging").textContent,
    ).toBe("—");
    expect(
      screen.getByTestId("variables-row-API_BASE-value-prod").textContent,
    ).toBe("—");
  });

  it("masks every value with bullets when isSecret is true", () => {
    renderWithProviders(
      <VariableListRow
        row={row({
          isSecret: true,
          scope: "personal",
          values: { local: "shhh", staging: "secret" },
        })}
        envColumnNames={["local", "staging"]}
      />,
    );
    expect(
      screen.getByTestId("variables-row-API_BASE-value-local").textContent,
    ).toBe("••••••••");
    expect(
      screen.getByTestId("variables-row-API_BASE-value-staging").textContent,
    ).toBe("••••••••");
    expect(
      screen.getByTestId("variables-row-API_BASE-lock"),
    ).toBeInTheDocument();
  });

  it("shows em-dash placeholders when fewer than 3 envs are passed", () => {
    renderWithProviders(
      <VariableListRow
        row={row({ values: { local: "x" } })}
        envColumnNames={["local"]}
      />,
    );
    // first env present + 2 placeholders
    expect(
      screen.getByTestId("variables-row-API_BASE-value-local").textContent,
    ).toBe("x");
  });

  it("shows em-dash for usesCount=0", () => {
    renderWithProviders(
      <VariableListRow
        row={row({ usesCount: 0 })}
        envColumnNames={["local", "staging", "prod"]}
      />,
    );
    expect(screen.getByTestId("variables-row-API_BASE-uses").textContent).toBe(
      "—",
    );
  });

  it("triggers onClick on click and on Enter / Space key", async () => {
    const onClick = vi.fn();
    renderWithProviders(
      <VariableListRow
        row={row()}
        envColumnNames={["local"]}
        onClick={onClick}
      />,
    );
    const user = userEvent.setup();
    const node = screen.getByTestId("variables-row-API_BASE");
    await user.click(node);
    expect(onClick).toHaveBeenCalledTimes(1);
    node.focus();
    await user.keyboard("{Enter}");
    expect(onClick).toHaveBeenCalledTimes(2);
    await user.keyboard(" ");
    expect(onClick).toHaveBeenCalledTimes(3);
  });

  it("marks itself selected via data-selected attribute", () => {
    renderWithProviders(
      <VariableListRow row={row()} envColumnNames={["local"]} selected />,
    );
    expect(
      screen
        .getByTestId("variables-row-API_BASE")
        .getAttribute("data-selected"),
    ).toBe("true");
  });

  it("uses the secret scope glyph when isSecret is true", () => {
    renderWithProviders(
      <VariableListRow
        row={row({ scope: "personal", isSecret: true })}
        envColumnNames={["local"]}
      />,
    );
    const grid = screen.getByTestId("variables-row-API_BASE");
    expect(grid.getAttribute("data-scope")).toBe("secret");
  });
});
