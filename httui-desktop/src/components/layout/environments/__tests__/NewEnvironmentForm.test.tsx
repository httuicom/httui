import { describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { NewEnvironmentForm } from "@/components/layout/environments/NewEnvironmentForm";
import { renderWithProviders, screen } from "@/test/render";

describe("NewEnvironmentForm", () => {
  it("renders the name input + Save/Cancel buttons + target hint", () => {
    renderWithProviders(<NewEnvironmentForm />);
    expect(screen.getByTestId("new-environment-name")).toBeInTheDocument();
    expect(screen.getByTestId("new-environment-save")).toBeInTheDocument();
    expect(screen.getByTestId("new-environment-cancel")).toBeInTheDocument();
    expect(
      screen.getByTestId("new-environment-target-hint").textContent,
    ).toMatch(/<nome>\.toml/);
  });

  it("shows the live target filename hint as the user types", async () => {
    renderWithProviders(<NewEnvironmentForm />);
    await userEvent
      .setup()
      .type(screen.getByTestId("new-environment-name"), "staging");
    expect(
      screen.getByTestId("new-environment-target-hint").textContent,
    ).toMatch(/staging\.toml/);
  });

  it("submits the trimmed name on Save click", async () => {
    const onSubmit = vi.fn();
    renderWithProviders(<NewEnvironmentForm onSubmit={onSubmit} />);
    const user = userEvent.setup();
    await user.type(screen.getByTestId("new-environment-name"), "  staging  ");
    await user.click(screen.getByTestId("new-environment-save"));
    expect(onSubmit).toHaveBeenCalledWith({ name: "staging" });
  });

  it("blocks submit on empty name and surfaces the error", async () => {
    const onSubmit = vi.fn();
    renderWithProviders(<NewEnvironmentForm onSubmit={onSubmit} />);
    await userEvent.setup().click(screen.getByTestId("new-environment-save"));
    expect(onSubmit).not.toHaveBeenCalled();
    expect(
      screen.getByTestId("new-environment-name-error").textContent,
    ).toMatch(/required/i);
  });

  it("blocks submit on duplicate name", async () => {
    const onSubmit = vi.fn();
    renderWithProviders(
      <NewEnvironmentForm
        existingFilenames={["staging.toml"]}
        onSubmit={onSubmit}
      />,
    );
    const user = userEvent.setup();
    await user.type(screen.getByTestId("new-environment-name"), "staging");
    await user.click(screen.getByTestId("new-environment-save"));
    expect(onSubmit).not.toHaveBeenCalled();
    expect(
      screen.getByTestId("new-environment-name-error").textContent,
    ).toMatch(/already exists/i);
  });

  it("submits on Enter when valid", async () => {
    const onSubmit = vi.fn();
    renderWithProviders(<NewEnvironmentForm onSubmit={onSubmit} />);
    await userEvent
      .setup()
      .type(screen.getByTestId("new-environment-name"), "stg{Enter}");
    expect(onSubmit).toHaveBeenCalledWith({ name: "stg" });
  });

  it("cancels on Escape inside the name input", async () => {
    const onCancel = vi.fn();
    renderWithProviders(<NewEnvironmentForm onCancel={onCancel} />);
    const user = userEvent.setup();
    await user.click(screen.getByTestId("new-environment-name"));
    await user.keyboard("{Escape}");
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("Cancel button fires onCancel without onSubmit", async () => {
    const onSubmit = vi.fn();
    const onCancel = vi.fn();
    renderWithProviders(
      <NewEnvironmentForm onSubmit={onSubmit} onCancel={onCancel} />,
    );
    await userEvent.setup().click(screen.getByTestId("new-environment-cancel"));
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(onSubmit).not.toHaveBeenCalled();
  });
});
