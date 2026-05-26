import { describe, expect, it } from "vitest";

import { VariableDetailHeader } from "@/components/layout/variables/VariableDetailHeader";
import type { VariableRow } from "@/components/layout/variables/variable-derive";
import { renderWithProviders, screen } from "@/test/render";

function row(over: Partial<VariableRow> = {}): VariableRow {
  return {
    key: "API_BASE",
    scope: "workspace",
    isSecret: false,
    values: { local: "http://localhost" },
    usesCount: 0,
    ...over,
  };
}

describe("VariableDetailHeader", () => {
  it("renders the variable key with the scope icon", () => {
    renderWithProviders(<VariableDetailHeader row={row()} />);
    expect(screen.getByTestId("variable-detail-header-key").textContent).toBe(
      "API_BASE",
    );
    // Lucide renders the icon as an inline SVG inside the slot.
    expect(
      screen.getByTestId("variable-detail-header-glyph").querySelector("svg"),
    ).not.toBeNull();
  });

  it("uses the secret scope when isSecret is true (regardless of scope discriminator)", () => {
    renderWithProviders(
      <VariableDetailHeader row={row({ scope: "personal", isSecret: true })} />,
    );
    expect(
      screen.getByTestId("variable-detail-header").getAttribute("data-scope"),
    ).toBe("secret");
    expect(
      screen.getByTestId("variable-detail-header-secret-chip"),
    ).toBeInTheDocument();
  });

  it("hides the secret chip when isSecret is false", () => {
    renderWithProviders(<VariableDetailHeader row={row()} />);
    expect(
      screen.queryByTestId("variable-detail-header-secret-chip"),
    ).not.toBeInTheDocument();
  });

  it("renders the scope label and hint in the subtitle", () => {
    renderWithProviders(<VariableDetailHeader row={row()} />);
    const hint = screen.getByTestId("variable-detail-header-hint").textContent;
    expect(hint).toMatch(/Workspace/);
    expect(hint).toMatch(/envs\/\*\.toml/);
  });
});
