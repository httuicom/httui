import { describe, it, expect } from "vitest";

import { renderWithProviders, screen } from "@/test/render";
import { VariablesDetailPanel } from "@/components/layout/variables/VariablesDetailPanel";

describe("VariablesDetailPanel", () => {
  it("renders the empty state when no key is selected", () => {
    renderWithProviders(<VariablesDetailPanel />);
    expect(screen.getByTestId("variables-detail-empty")).toBeInTheDocument();
    expect(screen.getByText(/Select a variable/)).toBeInTheDocument();
  });

  it("renders the empty state when key is selected but no children supplied", () => {
    renderWithProviders(<VariablesDetailPanel selectedKey="api_base" />);
    expect(screen.getByTestId("variables-detail-empty")).toBeInTheDocument();
  });

  it("renders the children slot when key + children are both present", () => {
    renderWithProviders(
      <VariablesDetailPanel selectedKey="api_base">
        <div data-testid="detail-stub" />
      </VariablesDetailPanel>,
    );
    expect(screen.getByTestId("detail-stub")).toBeInTheDocument();
    expect(
      screen.queryByTestId("variables-detail-empty"),
    ).not.toBeInTheDocument();
  });

  it("renders the panel container regardless of state", () => {
    renderWithProviders(<VariablesDetailPanel />);
    expect(screen.getByTestId("variables-detail-panel")).toBeInTheDocument();
  });
});
