import { describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { RenameEnvironmentForm } from "@/components/layout/environments/RenameEnvironmentForm";
import type { EnvironmentSummary } from "@/components/layout/environments/envs-meta";
import { renderWithProviders, screen } from "@/test/render";

function env(over: Partial<EnvironmentSummary> = {}): EnvironmentSummary {
  return {
    name: "staging",
    filename: "staging.toml",
    varCount: 0,
    connectionsUsedCount: 0,
    isActive: false,
    isPersonal: false,
    isTemporary: false,
    ...over,
  };
}

describe("RenameEnvironmentForm", () => {
  it("pre-fills the input with the current env name", () => {
    renderWithProviders(<RenameEnvironmentForm env={env()} />);
    expect(
      (screen.getByTestId("rename-environment-name") as HTMLInputElement).value,
    ).toBe("staging");
  });

  it("renders the source name in the heading", () => {
    renderWithProviders(<RenameEnvironmentForm env={env()} />);
    expect(
      screen.getByTestId("rename-environment-heading").textContent,
    ).toMatch(/staging/);
  });

  it("shows a target hint that reflects the typed name + suffix", async () => {
    renderWithProviders(<RenameEnvironmentForm env={env()} />);
    const user = userEvent.setup();
    const input = screen.getByTestId("rename-environment-name");
    await user.clear(input);
    await user.type(input, "stg2");
    expect(
      screen.getByTestId("rename-environment-target-hint").textContent,
    ).toMatch(/envs\/stg2\.toml/);
  });

  it("uses .local.toml in the hint for personal envs", async () => {
    renderWithProviders(
      <RenameEnvironmentForm
        env={env({
          name: "staging",
          filename: "staging.local.toml",
          isPersonal: true,
        })}
      />,
    );
    const user = userEvent.setup();
    const input = screen.getByTestId("rename-environment-name");
    await user.clear(input);
    await user.type(input, "stg2");
    expect(
      screen.getByTestId("rename-environment-target-hint").textContent,
    ).toMatch(/envs\/stg2\.local\.toml/);
  });

  it("submits with sourceFilename + newName on Save click", async () => {
    const onSubmit = vi.fn();
    renderWithProviders(
      <RenameEnvironmentForm env={env()} onSubmit={onSubmit} />,
    );
    const user = userEvent.setup();
    const input = screen.getByTestId("rename-environment-name");
    await user.clear(input);
    await user.type(input, "production");
    await user.click(screen.getByTestId("rename-environment-save"));
    expect(onSubmit).toHaveBeenCalledWith({
      sourceFilename: "staging.toml",
      newName: "production",
    });
  });

  it("treats no-change as cancel (does not call onSubmit)", async () => {
    const onSubmit = vi.fn();
    const onCancel = vi.fn();
    renderWithProviders(
      <RenameEnvironmentForm
        env={env()}
        onSubmit={onSubmit}
        onCancel={onCancel}
      />,
    );
    await userEvent
      .setup()
      .click(screen.getByTestId("rename-environment-save"));
    expect(onSubmit).not.toHaveBeenCalled();
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("blocks submit on empty name and surfaces the error", async () => {
    const onSubmit = vi.fn();
    renderWithProviders(
      <RenameEnvironmentForm env={env()} onSubmit={onSubmit} />,
    );
    const user = userEvent.setup();
    await user.clear(screen.getByTestId("rename-environment-name"));
    await user.click(screen.getByTestId("rename-environment-save"));
    expect(onSubmit).not.toHaveBeenCalled();
    expect(
      screen.getByTestId("rename-environment-name-error").textContent,
    ).toMatch(/required/i);
  });

  it("ignores the source filename when checking duplicates", async () => {
    const onSubmit = vi.fn();
    renderWithProviders(
      <RenameEnvironmentForm
        env={env()}
        existingFilenames={["staging.toml", "prod.toml"]}
        onSubmit={onSubmit}
      />,
    );
    const user = userEvent.setup();
    const input = screen.getByTestId("rename-environment-name");
    await user.clear(input);
    await user.type(input, "STAGING");
    await user.click(screen.getByTestId("rename-environment-save"));
    expect(onSubmit).toHaveBeenCalledWith({
      sourceFilename: "staging.toml",
      newName: "STAGING",
    });
  });

  it("blocks submit when target collides with another existing env", async () => {
    const onSubmit = vi.fn();
    renderWithProviders(
      <RenameEnvironmentForm
        env={env()}
        existingFilenames={["staging.toml", "prod.toml"]}
        onSubmit={onSubmit}
      />,
    );
    const user = userEvent.setup();
    const input = screen.getByTestId("rename-environment-name");
    await user.clear(input);
    await user.type(input, "prod");
    await user.click(screen.getByTestId("rename-environment-save"));
    expect(onSubmit).not.toHaveBeenCalled();
    expect(
      screen.getByTestId("rename-environment-name-error").textContent,
    ).toMatch(/already exists/i);
  });

  it("submits on Enter inside the input", async () => {
    const onSubmit = vi.fn();
    renderWithProviders(
      <RenameEnvironmentForm env={env()} onSubmit={onSubmit} />,
    );
    const user = userEvent.setup();
    const input = screen.getByTestId("rename-environment-name");
    await user.clear(input);
    await user.type(input, "stg2{Enter}");
    expect(onSubmit).toHaveBeenCalledWith({
      sourceFilename: "staging.toml",
      newName: "stg2",
    });
  });

  it("cancels on Escape inside the input", async () => {
    const onCancel = vi.fn();
    renderWithProviders(
      <RenameEnvironmentForm env={env()} onCancel={onCancel} />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("rename-environment-name"));
    await user.keyboard("{Escape}");
    expect(onCancel).toHaveBeenCalledTimes(1);
  });
});
