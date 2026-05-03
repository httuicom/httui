import { describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { CloneEnvironmentForm } from "@/components/layout/environments/CloneEnvironmentForm";
import { renderWithProviders, screen } from "@/test/render";

function setup(overrides: Record<string, unknown> = {}) {
  return renderWithProviders(
    <CloneEnvironmentForm
      sourceFilename="staging.toml"
      sourceName="staging"
      {...overrides}
    />,
  );
}

describe("CloneEnvironmentForm", () => {
  it("renders the source heading + name input + 4 checkboxes + buttons", () => {
    setup();
    expect(screen.getByTestId("clone-environment-heading").textContent).toMatch(
      /staging/,
    );
    expect(screen.getByTestId("clone-environment-name")).toBeInTheDocument();
    expect(screen.getByTestId("clone-copy-variables")).toBeInTheDocument();
    expect(
      screen.getByTestId("clone-copy-connections-used"),
    ).toBeInTheDocument();
    expect(screen.getByTestId("clone-mark-temporary")).toBeInTheDocument();
    expect(screen.getByTestId("clone-mark-personal")).toBeInTheDocument();
    expect(screen.getByTestId("clone-environment-save")).toBeInTheDocument();
    expect(screen.getByTestId("clone-environment-cancel")).toBeInTheDocument();
  });

  it("starts with Copy variables ON and the other three OFF", () => {
    setup();
    const find = (id: string) =>
      screen.getByTestId(id).querySelector("input") as HTMLInputElement;
    expect(find("clone-copy-variables").checked).toBe(true);
    expect(find("clone-copy-connections-used").checked).toBe(false);
    expect(find("clone-mark-temporary").checked).toBe(false);
    expect(find("clone-mark-personal").checked).toBe(false);
  });

  it("flips the target filename suffix when Mark personal is toggled", async () => {
    setup();
    const user = userEvent.setup();
    await user.type(screen.getByTestId("clone-environment-name"), "stg2");
    expect(screen.getByTestId("clone-target-hint").textContent).toMatch(
      /stg2\.toml/,
    );
    await user.click(screen.getByTestId("clone-mark-personal"));
    expect(screen.getByTestId("clone-target-hint").textContent).toMatch(
      /stg2\.local\.toml/,
    );
  });

  it("submits the parsed payload on Save click", async () => {
    const onSubmit = vi.fn();
    setup({ onSubmit });
    const user = userEvent.setup();
    await user.type(screen.getByTestId("clone-environment-name"), "stg-copy");
    await user.click(screen.getByTestId("clone-mark-temporary"));
    await user.click(screen.getByTestId("clone-environment-save"));
    expect(onSubmit).toHaveBeenCalledWith({
      sourceFilename: "staging.toml",
      name: "stg-copy",
      copyVariables: true,
      copyConnectionsUsed: false,
      markTemporary: true,
      markPersonal: false,
    });
  });

  it("blocks submit on empty name and surfaces the error", async () => {
    const onSubmit = vi.fn();
    setup({ onSubmit });
    await userEvent.setup().click(screen.getByTestId("clone-environment-save"));
    expect(onSubmit).not.toHaveBeenCalled();
    expect(
      screen.getByTestId("clone-environment-name-error").textContent,
    ).toMatch(/required/i);
  });

  it("blocks submit on duplicate (case-insensitive) against existingFilenames", async () => {
    const onSubmit = vi.fn();
    setup({
      onSubmit,
      existingFilenames: ["staging.toml", "Prod.toml"],
    });
    const user = userEvent.setup();
    await user.type(screen.getByTestId("clone-environment-name"), "PROD");
    await user.click(screen.getByTestId("clone-environment-save"));
    expect(onSubmit).not.toHaveBeenCalled();
    expect(
      screen.getByTestId("clone-environment-name-error").textContent,
    ).toMatch(/already exists/i);
  });

  it("toggles every checkbox independently", async () => {
    const onSubmit = vi.fn();
    setup({ onSubmit });
    const user = userEvent.setup();
    await user.type(screen.getByTestId("clone-environment-name"), "x");
    await user.click(screen.getByTestId("clone-copy-variables")); // OFF
    await user.click(screen.getByTestId("clone-copy-connections-used")); // ON
    await user.click(screen.getByTestId("clone-mark-temporary")); // ON
    await user.click(screen.getByTestId("clone-mark-personal")); // ON
    await user.click(screen.getByTestId("clone-environment-save"));
    expect(onSubmit).toHaveBeenCalledWith({
      sourceFilename: "staging.toml",
      name: "x",
      copyVariables: false,
      copyConnectionsUsed: true,
      markTemporary: true,
      markPersonal: true,
    });
  });

  it("submits on Enter inside the name input", async () => {
    const onSubmit = vi.fn();
    setup({ onSubmit });
    await userEvent
      .setup()
      .type(screen.getByTestId("clone-environment-name"), "stg2{Enter}");
    expect(onSubmit).toHaveBeenCalledWith({
      sourceFilename: "staging.toml",
      name: "stg2",
      copyVariables: true,
      copyConnectionsUsed: false,
      markTemporary: false,
      markPersonal: false,
    });
  });

  it("cancels on Escape inside the name input", async () => {
    const onCancel = vi.fn();
    setup({ onCancel });
    const user = userEvent.setup();
    await user.click(screen.getByTestId("clone-environment-name"));
    await user.keyboard("{Escape}");
    expect(onCancel).toHaveBeenCalledTimes(1);
  });
});
