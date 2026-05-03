import { describe, it, expect, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";

import { ConnectionsKindSidebar } from "@/components/layout/connections/ConnectionsKindSidebar";
import { CONNECTION_KIND_ORDER } from "@/components/layout/connections/connection-kinds";

function defaults() {
  return {
    countsByKind: { postgres: 3, mysql: 1 },
    selectedKind: null,
    onSelectKind: vi.fn(),
    envs: [
      { name: "local", status: "ok" as const, count: 4 },
      { name: "prod", status: "warn" as const, count: 2 },
    ],
  };
}

describe("ConnectionsKindSidebar", () => {
  it("renders one row per kind in canvas order", () => {
    renderWithProviders(<ConnectionsKindSidebar {...defaults()} />);
    for (const kind of CONNECTION_KIND_ORDER) {
      expect(screen.getByTestId(`kind-row-${kind}`)).toBeInTheDocument();
    }
  });

  it("shows the count next to each kind, defaulting to 0 when absent", () => {
    renderWithProviders(<ConnectionsKindSidebar {...defaults()} />);
    expect(screen.getByTestId("kind-row-postgres").textContent).toContain("3");
    expect(screen.getByTestId("kind-row-mysql").textContent).toContain("1");
    expect(screen.getByTestId("kind-row-mongo").textContent).toContain("0");
  });

  it("marks the selected row via data-selected", () => {
    renderWithProviders(
      <ConnectionsKindSidebar {...defaults()} selectedKind="postgres" />,
    );
    expect(
      screen.getByTestId("kind-row-postgres").getAttribute("data-selected"),
    ).toBe("true");
    expect(
      screen.getByTestId("kind-row-mysql").getAttribute("data-selected"),
    ).toBe("false");
  });

  it("clicking an unselected row dispatches with that kind", async () => {
    const onSelectKind = vi.fn();
    renderWithProviders(
      <ConnectionsKindSidebar {...defaults()} onSelectKind={onSelectKind} />,
    );
    await userEvent.setup().click(screen.getByTestId("kind-row-mysql"));
    expect(onSelectKind).toHaveBeenCalledWith("mysql");
  });

  it("clicking the already-selected row clears the filter (null)", async () => {
    const onSelectKind = vi.fn();
    renderWithProviders(
      <ConnectionsKindSidebar
        {...defaults()}
        selectedKind="postgres"
        onSelectKind={onSelectKind}
      />,
    );
    await userEvent.setup().click(screen.getByTestId("kind-row-postgres"));
    expect(onSelectKind).toHaveBeenCalledWith(null);
  });

  it("renders the per-environment section with status dots and counts", () => {
    renderWithProviders(<ConnectionsKindSidebar {...defaults()} />);
    expect(screen.getByTestId("env-row-local")).toBeInTheDocument();
    expect(screen.getByTestId("env-row-local").textContent).toContain("4");
    expect(screen.getByTestId("env-row-prod")).toBeInTheDocument();
    expect(screen.getByTestId("env-row-prod").textContent).toContain("2");
  });

  it("shows 'No environments' when envs is empty", () => {
    renderWithProviders(
      <ConnectionsKindSidebar {...defaults()} envs={[]} />,
    );
    expect(screen.getByText("No environments")).toBeInTheDocument();
  });

  it("renders the keychain hint card with the canvas copy", () => {
    renderWithProviders(<ConnectionsKindSidebar {...defaults()} />);
    const hint = screen.getByTestId("connections-keychain-hint");
    expect(hint.textContent).toContain("Local credentials");
    expect(hint.textContent).toContain("keychain");
  });
});
