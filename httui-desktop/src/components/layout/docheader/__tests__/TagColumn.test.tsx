import { describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { TagColumn } from "@/components/layout/docheader/TagColumn";
import { renderWithProviders, screen } from "@/test/render";

describe("TagColumn", () => {
  it("renders 'No tags' empty state when tags array is empty", () => {
    renderWithProviders(<TagColumn tags={[]} />);
    expect(screen.getByTestId("tag-column-empty")).toBeInTheDocument();
  });

  it("renders one chip per tag with the # prefix", () => {
    renderWithProviders(<TagColumn tags={["payments", "debug"]} />);
    expect(
      screen.getByTestId("tag-column-chip-payments-label").textContent,
    ).toBe("#payments");
    expect(screen.getByTestId("tag-column-chip-debug-label").textContent).toBe(
      "#debug",
    );
  });

  it("encodes tag count via data-tag-count", () => {
    renderWithProviders(<TagColumn tags={["a", "b", "c"]} />);
    expect(
      screen.getByTestId("tag-column").getAttribute("data-tag-count"),
    ).toBe("3");
  });

  it("makes chips interactive only when onSelectTag is supplied", async () => {
    const onSelectTag = vi.fn();
    renderWithProviders(<TagColumn tags={["a"]} onSelectTag={onSelectTag} />);
    const label = screen.getByTestId("tag-column-chip-a-label");
    expect(label.tagName).toBe("BUTTON");
    await userEvent.setup().click(label);
    expect(onSelectTag).toHaveBeenCalledWith("a");
  });

  it("renders chip labels as inert spans without onSelectTag", () => {
    renderWithProviders(<TagColumn tags={["a"]} />);
    expect(screen.getByTestId("tag-column-chip-a-label").tagName).toBe("SPAN");
  });

  it("hides + Add tag when no onAddTag is supplied", () => {
    renderWithProviders(<TagColumn tags={["a"]} />);
    expect(screen.queryByTestId("tag-column-add")).not.toBeInTheDocument();
  });

  it("opens the add form on + Add click and fires onAddTag on Enter", async () => {
    const onAddTag = vi.fn();
    renderWithProviders(<TagColumn tags={[]} onAddTag={onAddTag} />);
    const user = userEvent.setup();
    await user.click(screen.getByTestId("tag-column-add"));
    expect(screen.getByTestId("tag-column-add-form")).toBeInTheDocument();
    await user.type(
      screen.getByTestId("tag-column-add-input"),
      "new-tag{Enter}",
    );
    expect(onAddTag).toHaveBeenCalledWith("new-tag");
    expect(screen.queryByTestId("tag-column-add-form")).not.toBeInTheDocument();
  });

  it("does not fire onAddTag when the value is whitespace", async () => {
    const onAddTag = vi.fn();
    renderWithProviders(<TagColumn tags={[]} onAddTag={onAddTag} />);
    const user = userEvent.setup();
    await user.click(screen.getByTestId("tag-column-add"));
    await user.type(screen.getByTestId("tag-column-add-input"), "   {Enter}");
    expect(onAddTag).not.toHaveBeenCalled();
  });

  it("does not fire onAddTag when the tag is already applied", async () => {
    const onAddTag = vi.fn();
    renderWithProviders(<TagColumn tags={["dup"]} onAddTag={onAddTag} />);
    const user = userEvent.setup();
    await user.click(screen.getByTestId("tag-column-add"));
    await user.type(screen.getByTestId("tag-column-add-input"), "dup{Enter}");
    expect(onAddTag).not.toHaveBeenCalled();
  });

  it("Esc closes the add form without firing onAddTag", async () => {
    const onAddTag = vi.fn();
    renderWithProviders(<TagColumn tags={[]} onAddTag={onAddTag} />);
    const user = userEvent.setup();
    await user.click(screen.getByTestId("tag-column-add"));
    await user.type(screen.getByTestId("tag-column-add-input"), "x{Escape}");
    expect(onAddTag).not.toHaveBeenCalled();
    expect(screen.queryByTestId("tag-column-add-form")).not.toBeInTheDocument();
  });

  it("fires onRemoveTag on chip × click", async () => {
    const onRemoveTag = vi.fn();
    renderWithProviders(<TagColumn tags={["a"]} onRemoveTag={onRemoveTag} />);
    await userEvent
      .setup()
      .click(screen.getByTestId("tag-column-chip-a-remove"));
    expect(onRemoveTag).toHaveBeenCalledWith("a");
  });

  it("hides the × button when no onRemoveTag is supplied", () => {
    renderWithProviders(<TagColumn tags={["a"]} />);
    expect(
      screen.queryByTestId("tag-column-chip-a-remove"),
    ).not.toBeInTheDocument();
  });

  it("surfaces autocomplete suggestions filtered by input", async () => {
    renderWithProviders(
      <TagColumn
        tags={[]}
        availableTags={["payments", "debug", "release"]}
        onAddTag={() => {}}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("tag-column-add"));
    await user.type(screen.getByTestId("tag-column-add-input"), "de");
    expect(
      screen.getByTestId("tag-column-suggestion-debug"),
    ).toBeInTheDocument();
    expect(
      screen.queryByTestId("tag-column-suggestion-payments"),
    ).not.toBeInTheDocument();
  });

  it("clicking a suggestion adds the tag and closes the form", async () => {
    const onAddTag = vi.fn();
    renderWithProviders(
      <TagColumn
        tags={[]}
        availableTags={["payments", "debug"]}
        onAddTag={onAddTag}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("tag-column-add"));
    await user.type(screen.getByTestId("tag-column-add-input"), "pay");
    await user.click(screen.getByTestId("tag-column-suggestion-payments"));
    expect(onAddTag).toHaveBeenCalledWith("payments");
    expect(screen.queryByTestId("tag-column-add-form")).not.toBeInTheDocument();
  });

  it("hides suggestions for already-applied tags", async () => {
    renderWithProviders(
      <TagColumn
        tags={["debug"]}
        availableTags={["payments", "debug"]}
        onAddTag={() => {}}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("tag-column-add"));
    await user.type(screen.getByTestId("tag-column-add-input"), "de");
    expect(
      screen.queryByTestId("tag-column-suggestion-debug"),
    ).not.toBeInTheDocument();
  });

  it("disables every interactive control when busy", () => {
    renderWithProviders(
      <TagColumn
        tags={["a"]}
        onAddTag={() => {}}
        onRemoveTag={() => {}}
        busy
      />,
    );
    const add = screen.getByTestId("tag-column-add") as HTMLButtonElement;
    expect(add.disabled).toBe(true);
    expect(screen.getByTestId("tag-column").getAttribute("data-busy")).toBe(
      "true",
    );
  });
});
