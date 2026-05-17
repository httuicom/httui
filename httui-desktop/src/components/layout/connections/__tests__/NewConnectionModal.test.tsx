import { describe, it, expect, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { within } from "@testing-library/react";

import { renderWithProviders, screen } from "@/test/render";
import {
  NewConnectionModal,
  NEW_CONNECTION_TABS,
} from "@/components/layout/connections/NewConnectionModal";

function getHeader() {
  return within(screen.getByTestId("new-connection-form-header"));
}

describe("NewConnectionModal", () => {
  it("renders nothing when closed", () => {
    renderWithProviders(<NewConnectionModal open={false} onCancel={vi.fn()} />);
    expect(
      screen.queryByTestId("new-connection-modal"),
    ).not.toBeInTheDocument();
  });

  it("renders the modal with header, sidebar, tabs, body, footer", () => {
    renderWithProviders(<NewConnectionModal open onCancel={vi.fn()} />);
    expect(screen.getByTestId("new-connection-modal")).toBeInTheDocument();
    expect(
      screen.getByTestId("new-connection-kind-picker"),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId("new-connection-form-header"),
    ).toBeInTheDocument();
    expect(screen.getByTestId("new-connection-paste-hint")).toBeInTheDocument();
    expect(screen.getByTestId("new-connection-tabs")).toBeInTheDocument();
    expect(screen.getByTestId("new-connection-tab-body")).toBeInTheDocument();
    expect(screen.getByTestId("new-connection-footer")).toBeInTheDocument();
  });

  it("defaults to postgres header + Form tab", () => {
    renderWithProviders(<NewConnectionModal open onCancel={vi.fn()} />);
    expect(getHeader().getByText("PostgreSQL")).toBeInTheDocument();
    expect(
      screen.getByTestId("new-connection-placeholder-form"),
    ).toBeInTheDocument();
  });

  it("respects initialKind", () => {
    renderWithProviders(
      <NewConnectionModal open initialKind="mongo" onCancel={vi.fn()} />,
    );
    expect(getHeader().getByText("MongoDB")).toBeInTheDocument();
  });

  it("switching kind in the picker updates the header", async () => {
    renderWithProviders(<NewConnectionModal open onCancel={vi.fn()} />);
    await userEvent
      .setup()
      .click(screen.getByTestId("new-connection-kind-mysql"));
    expect(getHeader().getByText("MySQL / MariaDB")).toBeInTheDocument();
  });

  it("switching tabs swaps the placeholder", async () => {
    renderWithProviders(<NewConnectionModal open onCancel={vi.fn()} />);
    const sslTabId = NEW_CONNECTION_TABS.find((t) => t.id === "ssl")!.id;
    await userEvent
      .setup()
      .click(
        screen.getByText(
          NEW_CONNECTION_TABS.find((t) => t.id === sslTabId)!.label,
        ),
      );
    expect(
      screen.getByTestId("new-connection-placeholder-ssl"),
    ).toBeInTheDocument();
  });

  it("renderTabBody overrides the placeholder", () => {
    const renderTabBody = vi.fn(({ kind, tab }) => (
      <div data-testid="custom-body">{`${kind}/${tab}`}</div>
    ));
    renderWithProviders(
      <NewConnectionModal
        open
        onCancel={vi.fn()}
        renderTabBody={renderTabBody}
      />,
    );
    expect(screen.getByTestId("custom-body").textContent).toBe("postgres/form");
    expect(renderTabBody).toHaveBeenCalledWith({
      kind: "postgres",
      tab: "form",
    });
    expect(
      screen.queryByTestId("new-connection-placeholder-form"),
    ).not.toBeInTheDocument();
  });

  it("Escape key dispatches onCancel", async () => {
    const onCancel = vi.fn();
    renderWithProviders(<NewConnectionModal open onCancel={onCancel} />);
    await userEvent.setup().keyboard("{Escape}");
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("clicking the overlay dispatches onCancel", async () => {
    const onCancel = vi.fn();
    renderWithProviders(<NewConnectionModal open onCancel={onCancel} />);
    await userEvent
      .setup()
      .click(screen.getByTestId("new-connection-modal-overlay"));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("clicking inside the modal does NOT dispatch onCancel", async () => {
    const onCancel = vi.fn();
    renderWithProviders(<NewConnectionModal open onCancel={onCancel} />);
    await userEvent.setup().click(screen.getByTestId("new-connection-modal"));
    expect(onCancel).not.toHaveBeenCalled();
  });

  it("Cancel button dispatches onCancel", async () => {
    const onCancel = vi.fn();
    renderWithProviders(<NewConnectionModal open onCancel={onCancel} />);
    await userEvent.setup().click(screen.getByTestId("new-connection-cancel"));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("Save button dispatches onSave with current kind + tab", async () => {
    const onSave = vi.fn();
    renderWithProviders(
      <NewConnectionModal open onCancel={vi.fn()} onSave={onSave} />,
    );
    await userEvent.setup().click(screen.getByTestId("new-connection-save"));
    expect(onSave).toHaveBeenCalledWith({ kind: "postgres", tab: "form" });
  });

  it("Save reflects the active kind + tab after switching", async () => {
    const onSave = vi.fn();
    renderWithProviders(
      <NewConnectionModal open onCancel={vi.fn()} onSave={onSave} />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("new-connection-kind-mysql"));
    await user.click(
      screen.getByText(NEW_CONNECTION_TABS.find((t) => t.id === "ssl")!.label),
    );
    await user.click(screen.getByTestId("new-connection-save"));
    expect(onSave).toHaveBeenCalledWith({ kind: "mysql", tab: "ssl" });
  });

  it("Test button dispatches onTest", async () => {
    const onTest = vi.fn();
    renderWithProviders(
      <NewConnectionModal open onCancel={vi.fn()} onTest={onTest} />,
    );
    await userEvent.setup().click(screen.getByTestId("new-connection-test"));
    expect(onTest).toHaveBeenCalledWith({ kind: "postgres", tab: "form" });
  });

  it("disables Save when saveDisabled or onSave omitted", () => {
    const { rerender } = renderWithProviders(
      <NewConnectionModal open onCancel={vi.fn()} />,
    );
    expect(
      (screen.getByTestId("new-connection-save") as HTMLButtonElement).disabled,
    ).toBe(true);

    rerender(
      <NewConnectionModal
        open
        onCancel={vi.fn()}
        onSave={vi.fn()}
        saveDisabled
      />,
    );
    expect(
      (screen.getByTestId("new-connection-save") as HTMLButtonElement).disabled,
    ).toBe(true);
  });

  it("disables Test when onTest is omitted", () => {
    renderWithProviders(<NewConnectionModal open onCancel={vi.fn()} />);
    expect(
      (screen.getByTestId("new-connection-test") as HTMLButtonElement).disabled,
    ).toBe(true);
  });

  it("controlled kind: renders kindProp and routes picker selections via onKindChange", async () => {
    const onKindChange = vi.fn();
    const { rerender } = renderWithProviders(
      <NewConnectionModal
        open
        kind="postgres"
        onKindChange={onKindChange}
        onCancel={vi.fn()}
      />,
    );
    expect(getHeader().getByText("PostgreSQL")).toBeInTheDocument();
    await userEvent
      .setup()
      .click(screen.getByTestId("new-connection-kind-mysql"));
    expect(onKindChange).toHaveBeenCalledWith("mysql");
    // Header still reflects controlled prop until parent re-renders.
    expect(getHeader().getByText("PostgreSQL")).toBeInTheDocument();
    rerender(
      <NewConnectionModal
        open
        kind="mysql"
        onKindChange={onKindChange}
        onCancel={vi.fn()}
      />,
    );
    expect(getHeader().getByText("MySQL / MariaDB")).toBeInTheDocument();
  });

  it("controlled activeTab: routes tab clicks via onTabChange", async () => {
    const onTabChange = vi.fn();
    renderWithProviders(
      <NewConnectionModal
        open
        activeTab="form"
        onTabChange={onTabChange}
        onCancel={vi.fn()}
      />,
    );
    await userEvent
      .setup()
      .click(
        screen.getByText(
          NEW_CONNECTION_TABS.find((t) => t.id === "ssl")!.label,
        ),
      );
    expect(onTabChange).toHaveBeenCalledWith("ssl");
    // The placeholder still reflects the controlled prop ("form")
    // until the parent re-renders with the new value.
    expect(
      screen.getByTestId("new-connection-placeholder-form"),
    ).toBeInTheDocument();
  });
});
