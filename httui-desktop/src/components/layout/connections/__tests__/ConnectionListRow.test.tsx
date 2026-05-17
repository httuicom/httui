import { describe, it, expect, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";

import {
  ConnectionListRow,
  type ListRowItem,
} from "@/components/layout/connections/ConnectionListRow";

function item(overrides: Partial<ListRowItem> = {}): ListRowItem {
  return {
    id: "c1",
    name: "alpha",
    kind: "postgres",
    host: "db.local",
    env: "local",
    latencyMs: 12,
    status: "ok",
    uses: 4,
    isProd: false,
    ...overrides,
  };
}

describe("ConnectionListRow", () => {
  it("renders the kind icon for postgres", () => {
    renderWithProviders(
      <ConnectionListRow item={item()} selected={false} onSelect={() => {}} />,
    );
    const row = screen.getByTestId("connection-row-c1");
    expect(row.querySelector('[data-kind="postgres"]')).toBeTruthy();
  });

  it("renders fallback icon when kind is null (sqlite)", () => {
    renderWithProviders(
      <ConnectionListRow
        item={item({ id: "c2", kind: null })}
        selected={false}
        onSelect={() => {}}
      />,
    );
    expect(
      screen
        .getByTestId("connection-row-c2")
        .querySelector('[data-kind="unknown"]'),
    ).toBeTruthy();
  });

  it("shows the PROD chip when isProd is true", () => {
    renderWithProviders(
      <ConnectionListRow
        item={item({ id: "c3", name: "prod-db", isProd: true })}
        selected={false}
        onSelect={() => {}}
      />,
    );
    expect(screen.getByTestId("connection-row-c3-prod")).toBeInTheDocument();
  });

  it("hides the PROD chip when isProd is false", () => {
    renderWithProviders(
      <ConnectionListRow item={item()} selected={false} onSelect={() => {}} />,
    );
    expect(screen.queryByTestId("connection-row-c1-prod")).toBeNull();
  });

  it("renders host, env, and latency formatted as Nms", () => {
    renderWithProviders(
      <ConnectionListRow
        item={item({ host: "db.local", env: "staging", latencyMs: 47 })}
        selected={false}
        onSelect={() => {}}
      />,
    );
    const row = screen.getByTestId("connection-row-c1");
    expect(row.textContent).toContain("db.local");
    expect(row.textContent).toContain("staging");
    expect(row.textContent).toContain("47ms");
  });

  it("renders dash when host / env / latency missing", () => {
    renderWithProviders(
      <ConnectionListRow
        item={item({ host: null, env: null, latencyMs: null })}
        selected={false}
        onSelect={() => {}}
      />,
    );
    const row = screen.getByTestId("connection-row-c1");
    expect(row.textContent).toContain("—");
  });

  it("renders 'N uses' from item.uses", () => {
    renderWithProviders(
      <ConnectionListRow
        item={item({ uses: 17 })}
        selected={false}
        onSelect={() => {}}
      />,
    );
    expect(screen.getByTestId("connection-row-c1").textContent).toContain(
      "17 uses",
    );
  });

  it("clicking the row dispatches onSelect with the id", async () => {
    const onSelect = vi.fn();
    renderWithProviders(
      <ConnectionListRow item={item()} selected={false} onSelect={onSelect} />,
    );
    await userEvent.setup().click(screen.getByTestId("connection-row-c1"));
    expect(onSelect).toHaveBeenCalledWith("c1");
  });

  it("opens the row actions menu without bubbling to onSelect", async () => {
    const onSelect = vi.fn();
    const onEdit = vi.fn();
    renderWithProviders(
      <ConnectionListRow
        item={item()}
        selected={false}
        onSelect={onSelect}
        onEdit={onEdit}
      />,
    );
    await userEvent.setup().click(screen.getByTestId("connection-row-c1-more"));
    // Trigger click should not select the row.
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("does not render the actions menu when no handlers are provided", () => {
    renderWithProviders(
      <ConnectionListRow item={item()} selected={false} onSelect={vi.fn()} />,
    );
    expect(screen.queryByTestId("connection-row-c1-more")).toBeNull();
  });

  it("marks selected via data-selected", () => {
    renderWithProviders(
      <ConnectionListRow item={item()} selected={true} onSelect={() => {}} />,
    );
    expect(
      screen.getByTestId("connection-row-c1").getAttribute("data-selected"),
    ).toBe("true");
  });
});
