import { describe, expect, it } from "vitest";
import userEvent from "@testing-library/user-event";

import { ExplainPlan } from "@/components/blocks/db/ExplainPlan";
import {
  type PlanNode,
  formatRows,
} from "@/components/blocks/db/explain-plan-types";
import { renderWithProviders, screen } from "@/test/render";

function node(over: Partial<PlanNode> = {}): PlanNode {
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

describe("formatRows", () => {
  it("comma-groups large numbers in en-US locale", () => {
    expect(formatRows(1234567, "en-US")).toBe("1,234,567");
  });

  it("preserves small numbers", () => {
    expect(formatRows(50, "en-US")).toBe("50");
  });
});

describe("ExplainPlan", () => {
  it("renders the unsupported state with optional driver label", () => {
    renderWithProviders(
      <ExplainPlan plan={null} unsupported driverLabel="SQLite" />,
    );
    const root = screen.getByTestId("explain-plan");
    expect(root.getAttribute("data-state")).toBe("unsupported");
    expect(root.textContent).toMatch(/SQLite/);
  });

  it("renders the loading state when plan is null", () => {
    renderWithProviders(<ExplainPlan plan={null} />);
    expect(screen.getByTestId("explain-plan").getAttribute("data-state")).toBe(
      "loading",
    );
  });

  it("renders a single-node plan with op + cost + rows", () => {
    renderWithProviders(<ExplainPlan plan={node()} />);
    expect(screen.getByTestId("explain-plan").getAttribute("data-state")).toBe(
      "ready",
    );
    expect(screen.getByTestId("explain-plan-op").textContent).toBe("Limit");
    expect(screen.getByTestId("explain-plan-cost").textContent).toBe(
      "0.42..18.7",
    );
    expect(screen.getByTestId("explain-plan-rows").textContent).toBe("50");
  });

  it("renders the cost bar with the expected percentage", () => {
    renderWithProviders(<ExplainPlan plan={node({ pct: 75 })} />);
    expect(
      screen.getByTestId("explain-plan-cost-bar").getAttribute("data-pct"),
    ).toBe("75");
  });

  it("clamps pct above 100 and below 0 in the cost bar", () => {
    renderWithProviders(<ExplainPlan plan={node({ pct: 150 })} />);
    expect(
      screen.getByTestId("explain-plan-cost-bar").getAttribute("data-pct"),
    ).toBe("100");
  });

  it("flags warn nodes with the warning icon + data-warn", () => {
    renderWithProviders(<ExplainPlan plan={node({ warn: true })} />);
    expect(screen.getByTestId("explain-plan-warn-icon")).toBeInTheDocument();
    const planNode = screen.getByTestId("explain-plan-node");
    expect(planNode.getAttribute("data-warn")).toBe("true");
  });

  it("renders children at increased depth", () => {
    const root: PlanNode = node({
      children: [node({ op: "Sort" }), node({ op: "Hash" })],
    });
    renderWithProviders(<ExplainPlan plan={root} />);
    const nodes = screen.getAllByTestId("explain-plan-node");
    expect(nodes).toHaveLength(3);
    expect(nodes[1]!.getAttribute("data-depth")).toBe("1");
    expect(nodes[2]!.getAttribute("data-depth")).toBe("1");
  });

  it("hides the toggle on leaf nodes", () => {
    renderWithProviders(<ExplainPlan plan={node()} />);
    expect(
      screen.queryByTestId("explain-plan-toggle-0-Limit-(rows=50)"),
    ).not.toBeInTheDocument();
  });

  it("renders the toggle on parent nodes and collapses on click", async () => {
    const root: PlanNode = node({
      children: [node({ op: "Sort" })],
    });
    renderWithProviders(<ExplainPlan plan={root} />);
    // Initially open: 2 nodes rendered (root + child).
    expect(screen.getAllByTestId("explain-plan-node")).toHaveLength(2);
    const toggle = screen.getByTestId("explain-plan-toggle-0-Limit-(rows=50)");
    expect(toggle.textContent).toBe("▾");
    await userEvent.setup().click(toggle);
    // Collapsed: only the root remains visible.
    expect(screen.getAllByTestId("explain-plan-node")).toHaveLength(1);
  });

  it("hides the target text when target is empty", () => {
    renderWithProviders(<ExplainPlan plan={node({ target: "" })} />);
    expect(screen.queryByTestId("explain-plan-target")).not.toBeInTheDocument();
  });

  it("flags the last child via data-last", () => {
    const root: PlanNode = node({
      children: [node({ op: "First" }), node({ op: "Last" })],
    });
    renderWithProviders(<ExplainPlan plan={root} />);
    const nodes = screen.getAllByTestId("explain-plan-node");
    // root is implicitly last, plus the second child.
    expect(nodes[0]!.getAttribute("data-last")).toBe("true");
    expect(nodes[1]!.getAttribute("data-last")).toBeNull();
    expect(nodes[2]!.getAttribute("data-last")).toBe("true");
  });

  it("renders 6+ levels deep without crashing (acceptance criterion)", () => {
    let root: PlanNode = node({ op: "L6" });
    for (const op of ["L5", "L4", "L3", "L2", "L1"]) {
      root = node({ op, children: [root] });
    }
    renderWithProviders(<ExplainPlan plan={root} />);
    const nodes = screen.getAllByTestId("explain-plan-node");
    expect(nodes).toHaveLength(6);
    expect(nodes[5]!.getAttribute("data-depth")).toBe("5");
  });
});
