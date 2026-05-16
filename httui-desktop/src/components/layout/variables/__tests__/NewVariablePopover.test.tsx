import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { act } from "@testing-library/react";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";

import { NewVariablePopover } from "@/components/layout/variables/NewVariablePopover";
import { useNewVariablePopoverStore } from "@/stores/newVariablePopover";
import { useEnvironmentStore } from "@/stores/environment";

const setVariable = vi.fn(async () => ({}) as never);

function openPopover() {
  act(() => {
    useNewVariablePopoverStore.getState().openForm();
  });
}

beforeEach(() => {
  setVariable.mockClear();
  useNewVariablePopoverStore.setState({ open: false });
  useEnvironmentStore.setState({
    activeEnvironment: { id: "e1", name: "local", is_active: true },
    setVariable,
  } as never);
});
afterEach(() => useNewVariablePopoverStore.setState({ open: false }));

describe("NewVariablePopover", () => {
  it("renders nothing while closed", () => {
    renderWithProviders(<NewVariablePopover />);
    expect(screen.queryByTestId("new-variable-popover")).toBeNull();
  });

  it("opens via the store and targets the active env", () => {
    renderWithProviders(<NewVariablePopover />);
    openPopover();
    expect(screen.getByTestId("new-variable-popover")).toBeInTheDocument();
    expect(screen.getByText("into local")).toBeInTheDocument();
  });

  it("saves a plain variable", async () => {
    const user = userEvent.setup();
    renderWithProviders(<NewVariablePopover />);
    openPopover();
    await user.type(screen.getByTestId("new-variable-name"), "API_BASE");
    await user.type(screen.getByTestId("new-variable-value"), "http://x");
    await user.click(screen.getByTestId("new-variable-save"));
    expect(setVariable).toHaveBeenCalledWith(
      "e1",
      "API_BASE",
      "http://x",
      false,
    );
    expect(useNewVariablePopoverStore.getState().open).toBe(false);
  });

  it("Secret type sets is_secret on save", async () => {
    const user = userEvent.setup();
    renderWithProviders(<NewVariablePopover />);
    openPopover();
    await user.type(screen.getByTestId("new-variable-name"), "TOKEN");
    await user.click(screen.getByTestId("new-variable-type-Secret"));
    await user.type(screen.getByTestId("new-variable-value"), "s3cret");
    await user.click(screen.getByTestId("new-variable-save"));
    expect(setVariable).toHaveBeenCalledWith("e1", "TOKEN", "s3cret", true);
  });

  it("template helpers append into the value field", async () => {
    const user = userEvent.setup();
    renderWithProviders(<NewVariablePopover />);
    openPopover();
    await user.click(screen.getByTestId("new-variable-helper-{{uuid()}}"));
    await user.click(
      screen.getByTestId("new-variable-helper-{{$prev.body.id}}"),
    );
    expect(
      (screen.getByTestId("new-variable-value") as HTMLInputElement).value,
    ).toBe("{{uuid()}}{{$prev.body.id}}");
  });

  it("blocks save with an empty name", async () => {
    const user = userEvent.setup();
    renderWithProviders(<NewVariablePopover />);
    openPopover();
    expect(screen.getByTestId("new-variable-save")).toBeDisabled();
    await user.type(screen.getByTestId("new-variable-name"), "X");
    expect(screen.getByTestId("new-variable-save")).not.toBeDisabled();
  });

  it("Escape cancels", async () => {
    const user = userEvent.setup();
    renderWithProviders(<NewVariablePopover />);
    openPopover();
    expect(screen.getByTestId("new-variable-popover")).toBeInTheDocument();
    await user.keyboard("{Escape}");
    expect(useNewVariablePopoverStore.getState().open).toBe(false);
  });

  it("Cancel button closes", async () => {
    const user = userEvent.setup();
    renderWithProviders(<NewVariablePopover />);
    openPopover();
    await user.click(screen.getByTestId("new-variable-cancel"));
    expect(useNewVariablePopoverStore.getState().open).toBe(false);
  });

  it("surfaces an error when there is no active environment", async () => {
    const user = userEvent.setup();
    useEnvironmentStore.setState({
      activeEnvironment: null,
      setVariable,
    } as never);
    renderWithProviders(<NewVariablePopover />);
    openPopover();
    expect(screen.getByText(/no active environment/i)).toBeInTheDocument();
    await user.type(screen.getByTestId("new-variable-name"), "X");
    expect(screen.getByTestId("new-variable-save")).toBeDisabled();
  });
});
