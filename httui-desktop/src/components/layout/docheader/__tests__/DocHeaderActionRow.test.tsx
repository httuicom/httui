import { describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { DocHeaderActionRow } from "@/components/layout/docheader/DocHeaderActionRow";
import { renderWithProviders, screen } from "@/test/render";

describe("DocHeaderActionRow", () => {
  it("renders an empty action row when no handlers are provided", () => {
    renderWithProviders(<DocHeaderActionRow />);
    const row = screen.getByTestId("docheader-action-row");
    expect(row).toBeInTheDocument();
    expect(
      screen.queryByTestId("docheader-action-run-all"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByTestId("docheader-action-share"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByTestId("docheader-action-overflow"),
    ).not.toBeInTheDocument();
  });

  it("renders the Run-all button and fires onRunAll on click", async () => {
    const onRunAll = vi.fn();
    renderWithProviders(<DocHeaderActionRow onRunAll={onRunAll} />);
    const btn = screen.getByTestId("docheader-action-run-all");
    expect(btn.textContent).toMatch(/Run all/);
    await userEvent.setup().click(btn);
    expect(onRunAll).toHaveBeenCalledTimes(1);
  });

  it("disables the Run-all button when busy", () => {
    renderWithProviders(<DocHeaderActionRow onRunAll={() => {}} runAllBusy />);
    const btn = screen.getByTestId(
      "docheader-action-run-all",
    ) as HTMLButtonElement;
    expect(btn.disabled).toBe(true);
    expect(btn.getAttribute("data-busy")).toBe("true");
  });

  it("renders the Share button and fires onShare on click", async () => {
    const onShare = vi.fn();
    renderWithProviders(<DocHeaderActionRow onShare={onShare} />);
    await userEvent.setup().click(screen.getByTestId("docheader-action-share"));
    expect(onShare).toHaveBeenCalledTimes(1);
  });

  it("hides the overflow trigger when no overflow handler is provided", () => {
    renderWithProviders(
      <DocHeaderActionRow onRunAll={() => {}} onShare={() => {}} />,
    );
    expect(
      screen.queryByTestId("docheader-action-overflow"),
    ).not.toBeInTheDocument();
  });

  it("shows the overflow trigger when at least one overflow handler is set", () => {
    renderWithProviders(<DocHeaderActionRow onDuplicate={() => {}} />);
    expect(screen.getByTestId("docheader-action-overflow")).toBeInTheDocument();
  });

  it("opens the overflow menu on trigger click", async () => {
    renderWithProviders(<DocHeaderActionRow onDuplicate={() => {}} />);
    expect(
      screen.queryByTestId("docheader-action-overflow-menu"),
    ).not.toBeInTheDocument();
    await userEvent
      .setup()
      .click(screen.getByTestId("docheader-action-overflow"));
    expect(
      screen.getByTestId("docheader-action-overflow-menu"),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId("docheader-action-overflow").getAttribute("data-open"),
    ).toBe("true");
  });

  it("renders only menu items whose handlers are provided", async () => {
    renderWithProviders(<DocHeaderActionRow onDelete={() => {}} />);
    await userEvent
      .setup()
      .click(screen.getByTestId("docheader-action-overflow"));
    expect(screen.getByTestId("docheader-action-delete")).toBeInTheDocument();
    expect(
      screen.queryByTestId("docheader-action-duplicate"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByTestId("docheader-action-archive"),
    ).not.toBeInTheDocument();
  });

  it("fires the right overflow handler and closes the menu", async () => {
    const onDuplicate = vi.fn();
    renderWithProviders(<DocHeaderActionRow onDuplicate={onDuplicate} />);
    const user = userEvent.setup();
    await user.click(screen.getByTestId("docheader-action-overflow"));
    await user.click(screen.getByTestId("docheader-action-duplicate"));
    expect(onDuplicate).toHaveBeenCalledTimes(1);
    expect(
      screen.queryByTestId("docheader-action-overflow-menu"),
    ).not.toBeInTheDocument();
  });

  it("flags the Delete item with error tone", async () => {
    renderWithProviders(<DocHeaderActionRow onDelete={() => {}} />);
    await userEvent
      .setup()
      .click(screen.getByTestId("docheader-action-overflow"));
    expect(
      screen.getByTestId("docheader-action-delete").getAttribute("data-tone"),
    ).toBe("error");
  });

  it("toggles the menu closed when overflow is clicked again", async () => {
    renderWithProviders(<DocHeaderActionRow onDuplicate={() => {}} />);
    const user = userEvent.setup();
    const trigger = screen.getByTestId("docheader-action-overflow");
    await user.click(trigger);
    expect(
      screen.getByTestId("docheader-action-overflow-menu"),
    ).toBeInTheDocument();
    await user.click(trigger);
    expect(
      screen.queryByTestId("docheader-action-overflow-menu"),
    ).not.toBeInTheDocument();
  });

  it("fires onArchive when the Archive item is clicked", async () => {
    const onArchive = vi.fn();
    renderWithProviders(<DocHeaderActionRow onArchive={onArchive} />);
    const user = userEvent.setup();
    await user.click(screen.getByTestId("docheader-action-overflow"));
    await user.click(screen.getByTestId("docheader-action-archive"));
    expect(onArchive).toHaveBeenCalledTimes(1);
  });

  it("fires onDelete when the Delete item is clicked", async () => {
    const onDelete = vi.fn();
    renderWithProviders(<DocHeaderActionRow onDelete={onDelete} />);
    const user = userEvent.setup();
    await user.click(screen.getByTestId("docheader-action-overflow"));
    await user.click(screen.getByTestId("docheader-action-delete"));
    expect(onDelete).toHaveBeenCalledTimes(1);
    // Menu closes after firing.
    expect(
      screen.queryByTestId("docheader-action-overflow-menu"),
    ).not.toBeInTheDocument();
  });

  it("renders all three overflow items together when all handlers are provided", async () => {
    renderWithProviders(
      <DocHeaderActionRow
        onDuplicate={() => {}}
        onArchive={() => {}}
        onDelete={() => {}}
      />,
    );
    await userEvent
      .setup()
      .click(screen.getByTestId("docheader-action-overflow"));
    expect(
      screen.getByTestId("docheader-action-duplicate"),
    ).toBeInTheDocument();
    expect(screen.getByTestId("docheader-action-archive")).toBeInTheDocument();
    expect(screen.getByTestId("docheader-action-delete")).toBeInTheDocument();
  });

  it("sets aria-haspopup and aria-expanded on the overflow trigger", () => {
    renderWithProviders(<DocHeaderActionRow onDuplicate={() => {}} />);
    const trigger = screen.getByTestId("docheader-action-overflow");
    expect(trigger.getAttribute("aria-haspopup")).toBe("menu");
    expect(trigger.getAttribute("aria-expanded")).toBe("false");
  });
});
