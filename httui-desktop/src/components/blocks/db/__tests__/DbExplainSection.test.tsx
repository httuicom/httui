import { describe, expect, it } from "vitest";

import { DbExplainSection } from "@/components/blocks/db/DbExplainSection";
import type { PlanNode } from "@/components/blocks/db/explain-plan-types";
import { renderWithProviders, screen } from "@/test/render";

function leaf(over: Partial<PlanNode> = {}): PlanNode {
  return {
    op: "Limit",
    target: "(rows=50)",
    cost: "0.42..18.7",
    rows: 50,
    pct: 100,
    warn: false,
    children: [],
    ...over,
  };
}

describe("DbExplainSection", () => {
  it("returns null when plan is undefined and not unsupported (hide-entirely)", () => {
    const { container } = renderWithProviders(
      <DbExplainSection plan={undefined} />,
    );
    expect(container.firstChild).toBeNull();
    expect(screen.queryByTestId("db-explain-section")).not.toBeInTheDocument();
  });

  it("renders the header with default sub-label and the loading ExplainPlan when plan is null", () => {
    renderWithProviders(<DbExplainSection plan={null} />);
    expect(screen.getByTestId("db-explain-section-label").textContent).toBe(
      "EXPLAIN ANALYZE",
    );
    expect(screen.getByTestId("db-explain-section-sub").textContent).toBe(
      "buffers · timing",
    );
    expect(screen.getByTestId("explain-plan").getAttribute("data-state")).toBe(
      "loading",
    );
  });

  it("renders a custom subLabel when supplied", () => {
    renderWithProviders(
      <DbExplainSection plan={null} subLabel="cost · timing" />,
    );
    expect(screen.getByTestId("db-explain-section-sub").textContent).toBe(
      "cost · timing",
    );
  });

  it("renders the ready ExplainPlan and the summary annotation when plan is a node", () => {
    renderWithProviders(
      <DbExplainSection
        plan={leaf({ op: "Index Scan", target: "idx_route_provider" })}
        summary="uses idx_route_provider"
      />,
    );
    expect(screen.getByTestId("explain-plan").getAttribute("data-state")).toBe(
      "ready",
    );
    const summary = screen.getByTestId("db-explain-section-summary");
    expect(summary.textContent).toBe("uses idx_route_provider");
    expect(summary.getAttribute("title")).toBe("uses idx_route_provider");
    expect(screen.getByTestId("explain-plan-op").textContent).toBe(
      "Index Scan",
    );
  });

  it("hides the summary annotation when not supplied", () => {
    renderWithProviders(<DbExplainSection plan={leaf()} />);
    expect(
      screen.queryByTestId("db-explain-section-summary"),
    ).not.toBeInTheDocument();
  });

  it("renders the unsupported state with driver label even when plan is undefined", () => {
    renderWithProviders(
      <DbExplainSection plan={undefined} unsupported driverLabel="SQLite" />,
    );
    // Section is visible (the unsupported branch overrides the hide rule).
    expect(screen.getByTestId("db-explain-section")).toBeInTheDocument();
    expect(screen.getByTestId("explain-plan").getAttribute("data-state")).toBe(
      "unsupported",
    );
    expect(screen.getByTestId("explain-plan").textContent).toMatch(/SQLite/);
  });

  it("forwards unsupported state when plan is null too (consumer sets both during a transition)", () => {
    renderWithProviders(
      <DbExplainSection plan={null} unsupported driverLabel="BigQuery" />,
    );
    expect(screen.getByTestId("explain-plan").getAttribute("data-state")).toBe(
      "unsupported",
    );
  });
});
