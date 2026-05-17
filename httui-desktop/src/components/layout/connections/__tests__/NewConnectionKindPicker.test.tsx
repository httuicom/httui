import { describe, it, expect, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { renderWithProviders, screen } from "@/test/render";
import { NewConnectionKindPicker } from "@/components/layout/connections/NewConnectionKindPicker";
import { CONNECTION_KIND_ORDER } from "@/components/layout/connections/connection-kinds";

describe("NewConnectionKindPicker", () => {
  it("renders header copy from canvas spec", () => {
    renderWithProviders(
      <NewConnectionKindPicker
        selectedKind="postgres"
        onSelectKind={vi.fn()}
      />,
    );
    expect(screen.getByText("New connection")).toBeInTheDocument();
    expect(screen.getByText("Pick the kind")).toBeInTheDocument();
  });

  it("renders one row per kind in canvas order", () => {
    renderWithProviders(
      <NewConnectionKindPicker
        selectedKind="postgres"
        onSelectKind={vi.fn()}
      />,
    );
    for (const kind of CONNECTION_KIND_ORDER) {
      expect(
        screen.getByTestId(`new-connection-kind-${kind}`),
      ).toBeInTheDocument();
    }
  });

  it("marks the selected row via data-selected and aria-pressed", () => {
    renderWithProviders(
      <NewConnectionKindPicker selectedKind="mysql" onSelectKind={vi.fn()} />,
    );
    const mysql = screen.getByTestId("new-connection-kind-mysql");
    expect(mysql.getAttribute("data-selected")).toBe("true");
    expect(mysql.getAttribute("aria-pressed")).toBe("true");

    const postgres = screen.getByTestId("new-connection-kind-postgres");
    expect(postgres.getAttribute("data-selected")).toBe("false");
  });

  it("clicking a kind dispatches onSelectKind", async () => {
    const onSelectKind = vi.fn();
    renderWithProviders(
      <NewConnectionKindPicker
        selectedKind="postgres"
        onSelectKind={onSelectKind}
      />,
    );
    await userEvent
      .setup()
      .click(screen.getByTestId("new-connection-kind-mongo"));
    expect(onSelectKind).toHaveBeenCalledWith("mongo");
  });

  it("clicking the already-selected row still dispatches the same kind", async () => {
    // Modal pick-kind always commits to a kind; unlike the page sidebar
    // there is no "all" / null state. Clicking the active row is a no-op
    // for the consumer state but the dispatch still fires (the modal
    // form may use the dispatch as a "reset to defaults" hook later).
    const onSelectKind = vi.fn();
    renderWithProviders(
      <NewConnectionKindPicker
        selectedKind="postgres"
        onSelectKind={onSelectKind}
      />,
    );
    await userEvent
      .setup()
      .click(screen.getByTestId("new-connection-kind-postgres"));
    expect(onSelectKind).toHaveBeenCalledWith("postgres");
  });
});
