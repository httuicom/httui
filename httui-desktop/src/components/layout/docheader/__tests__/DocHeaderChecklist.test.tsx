import { describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { DocHeaderChecklist } from "@/components/layout/docheader/DocHeaderChecklist";
import { renderWithProviders, screen } from "@/test/render";

describe("DocHeaderChecklist", () => {
  it("renders nothing when there are no items and no save handler", () => {
    renderWithProviders(<DocHeaderChecklist items={[]} />);
    expect(screen.queryByTestId("docheader-checklist")).not.toBeInTheDocument();
  });

  it("renders read-only rows when no save handler is provided", () => {
    renderWithProviders(
      <DocHeaderChecklist
        items={[
          { text: "Verify", done: false },
          { text: "Done", done: true },
        ]}
      />,
    );
    expect(screen.getByTestId("docheader-checklist")).toBeInTheDocument();
    const rows = screen.getAllByTestId("docheader-checklist-row");
    expect(rows).toHaveLength(2);
    expect(
      screen.queryByTestId("docheader-checklist-remove"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByTestId("docheader-checklist-add"),
    ).not.toBeInTheDocument();
  });

  it("toggles done state when the checkbox is clicked", async () => {
    const onChecklistSave = vi.fn();
    renderWithProviders(
      <DocHeaderChecklist
        items={[
          { text: "First", done: false },
          { text: "Second", done: true },
        ]}
        onChecklistSave={onChecklistSave}
      />,
    );
    const checkboxes = screen.getAllByTestId("docheader-checklist-checkbox");
    await userEvent.setup().click(checkboxes[0]!);
    expect(onChecklistSave).toHaveBeenCalledWith([
      { text: "First", done: true },
      { text: "Second", done: true },
    ]);
  });

  it("removes a row when the × button is clicked", async () => {
    const onChecklistSave = vi.fn();
    renderWithProviders(
      <DocHeaderChecklist
        items={[
          { text: "First", done: false },
          { text: "Second", done: false },
        ]}
        onChecklistSave={onChecklistSave}
      />,
    );
    const removes = screen.getAllByTestId("docheader-checklist-remove");
    await userEvent.setup().click(removes[1]!);
    expect(onChecklistSave).toHaveBeenCalledWith([
      { text: "First", done: false },
    ]);
  });

  it("opens the add input on click and commits on Enter", async () => {
    const onChecklistSave = vi.fn();
    const user = userEvent.setup();
    renderWithProviders(
      <DocHeaderChecklist items={[]} onChecklistSave={onChecklistSave} />,
    );

    await user.click(screen.getByTestId("docheader-checklist-add"));
    const input = screen.getByTestId(
      "docheader-checklist-add-input",
    ) as HTMLInputElement;
    await user.type(input, "Verify the thing");
    await user.keyboard("{Enter}");
    expect(onChecklistSave).toHaveBeenCalledWith([
      { text: "Verify the thing", done: false },
    ]);
  });

  it("cancels the add input on Escape without committing", async () => {
    const onChecklistSave = vi.fn();
    const user = userEvent.setup();
    renderWithProviders(
      <DocHeaderChecklist items={[]} onChecklistSave={onChecklistSave} />,
    );

    await user.click(screen.getByTestId("docheader-checklist-add"));
    const input = screen.getByTestId(
      "docheader-checklist-add-input",
    ) as HTMLInputElement;
    await user.type(input, "Cancelled");
    await user.keyboard("{Escape}");
    expect(onChecklistSave).not.toHaveBeenCalled();
  });

  it("edits an item's text on click → blur", async () => {
    const onChecklistSave = vi.fn();
    const user = userEvent.setup();
    renderWithProviders(
      <DocHeaderChecklist
        items={[{ text: "Old", done: false }]}
        onChecklistSave={onChecklistSave}
      />,
    );
    await user.click(screen.getByTestId("docheader-checklist-text"));
    const input = screen.getByTestId(
      "docheader-checklist-text-input",
    ) as HTMLInputElement;
    await user.clear(input);
    await user.type(input, "New text");
    input.blur();
    expect(onChecklistSave).toHaveBeenCalledWith([
      { text: "New text", done: false },
    ]);
  });
});
