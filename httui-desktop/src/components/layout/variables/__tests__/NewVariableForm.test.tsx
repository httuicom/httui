import { describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { NewVariableForm } from "@/components/layout/variables/NewVariableForm";
import { renderWithProviders, screen } from "@/test/render";

describe("NewVariableForm", () => {
  it("renders the three fields with the active env label and Save/Cancel buttons", () => {
    renderWithProviders(<NewVariableForm activeEnv="local" />);
    expect(screen.getByTestId("new-variable-name")).toBeInTheDocument();
    expect(screen.getByTestId("new-variable-value")).toBeInTheDocument();
    expect(screen.getByTestId("new-variable-is-secret")).toBeInTheDocument();
    expect(screen.getByTestId("new-variable-active-env").textContent).toBe(
      "local",
    );
    expect(screen.getByTestId("new-variable-save")).toBeInTheDocument();
    expect(screen.getByTestId("new-variable-cancel")).toBeInTheDocument();
  });

  it("starts with is_secret OFF and 'Public' label", () => {
    renderWithProviders(<NewVariableForm activeEnv="local" />);
    expect(
      screen.getByTestId("new-variable-is-secret-label").textContent,
    ).toMatch(/Public/);
  });

  it("flips the is_secret label when toggled", async () => {
    renderWithProviders(<NewVariableForm activeEnv="local" />);
    await userEvent.setup().click(screen.getByTestId("new-variable-is-secret"));
    expect(
      screen.getByTestId("new-variable-is-secret-label").textContent,
    ).toMatch(/Secret/);
  });

  it("submits the trimmed name with value + isSecret + active env on Save click", async () => {
    const onSubmit = vi.fn();
    renderWithProviders(
      <NewVariableForm activeEnv="staging" onSubmit={onSubmit} />,
    );
    const user = userEvent.setup();
    await user.type(screen.getByTestId("new-variable-name"), "  API_BASE  ");
    await user.type(
      screen.getByTestId("new-variable-value"),
      "http://stg.example",
    );
    await user.click(screen.getByTestId("new-variable-is-secret"));
    await user.click(screen.getByTestId("new-variable-save"));
    expect(onSubmit).toHaveBeenCalledWith({
      name: "API_BASE",
      value: "http://stg.example",
      isSecret: true,
      env: "staging",
    });
  });

  it("blocks submit with an empty name and surfaces the validation error", async () => {
    const onSubmit = vi.fn();
    renderWithProviders(
      <NewVariableForm activeEnv="local" onSubmit={onSubmit} />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("new-variable-save"));
    expect(onSubmit).not.toHaveBeenCalled();
    expect(screen.getByTestId("new-variable-name-error").textContent).toMatch(
      /required/i,
    );
  });

  it("blocks submit on duplicate name (case-insensitive)", async () => {
    const onSubmit = vi.fn();
    renderWithProviders(
      <NewVariableForm
        activeEnv="local"
        existingNames={["API_BASE"]}
        onSubmit={onSubmit}
      />,
    );
    const user = userEvent.setup();
    await user.type(screen.getByTestId("new-variable-name"), "api_base");
    await user.click(screen.getByTestId("new-variable-save"));
    expect(onSubmit).not.toHaveBeenCalled();
    expect(screen.getByTestId("new-variable-name-error").textContent).toMatch(
      /already exists/i,
    );
  });

  it("submits on Enter inside the value input when the name is valid", async () => {
    const onSubmit = vi.fn();
    renderWithProviders(
      <NewVariableForm activeEnv="local" onSubmit={onSubmit} />,
    );
    const user = userEvent.setup();
    await user.type(screen.getByTestId("new-variable-name"), "X");
    await user.type(screen.getByTestId("new-variable-value"), "y{Enter}");
    expect(onSubmit).toHaveBeenCalledWith({
      name: "X",
      value: "y",
      isSecret: false,
      env: "local",
    });
  });

  it("cancels on Escape inside the name input", async () => {
    const onCancel = vi.fn();
    renderWithProviders(
      <NewVariableForm activeEnv="local" onCancel={onCancel} />,
    );
    const user = userEvent.setup();
    const name = screen.getByTestId("new-variable-name");
    await user.click(name);
    await user.keyboard("{Escape}");
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("cancels on Escape inside the value input", async () => {
    const onCancel = vi.fn();
    renderWithProviders(
      <NewVariableForm activeEnv="local" onCancel={onCancel} />,
    );
    const user = userEvent.setup();
    const value = screen.getByTestId("new-variable-value");
    await user.click(value);
    await user.keyboard("{Escape}");
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("Cancel button fires onCancel without dispatching onSubmit", async () => {
    const onSubmit = vi.fn();
    const onCancel = vi.fn();
    renderWithProviders(
      <NewVariableForm
        activeEnv="local"
        onSubmit={onSubmit}
        onCancel={onCancel}
      />,
    );
    await userEvent.setup().click(screen.getByTestId("new-variable-cancel"));
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(onSubmit).not.toHaveBeenCalled();
  });
});
