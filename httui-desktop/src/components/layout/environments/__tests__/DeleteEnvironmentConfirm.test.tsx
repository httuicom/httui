import { describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { DeleteEnvironmentConfirm } from "@/components/layout/environments/DeleteEnvironmentConfirm";
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

describe("DeleteEnvironmentConfirm", () => {
  it("renders the env name in the heading", () => {
    renderWithProviders(<DeleteEnvironmentConfirm env={env()} />);
    expect(
      screen.getByTestId("delete-environment-confirm-heading").textContent,
    ).toMatch(/staging/);
  });

  it("warns about .local.toml siblings for non-personal envs", () => {
    renderWithProviders(<DeleteEnvironmentConfirm env={env()} />);
    const body = screen.getByTestId(
      "delete-environment-confirm-body",
    ).textContent;
    expect(body).toMatch(/envs\/staging\.toml/);
    expect(body).toMatch(/\.local\.toml/);
  });

  it("uses a per-machine tone for personal envs (no sibling warning)", () => {
    renderWithProviders(
      <DeleteEnvironmentConfirm
        env={env({
          name: "staging",
          filename: "staging.local.toml",
          isPersonal: true,
        })}
      />,
    );
    const body = screen.getByTestId(
      "delete-environment-confirm-body",
    ).textContent;
    expect(body).toMatch(/staging\.local\.toml/);
    expect(body).toMatch(/gitignored/);
  });

  it("blocks Delete until the user types the env name into the confirm input", async () => {
    const onConfirm = vi.fn();
    renderWithProviders(
      <DeleteEnvironmentConfirm env={env()} onConfirm={onConfirm} />,
    );
    const submit = screen.getByTestId(
      "delete-environment-confirm-submit",
    ) as HTMLButtonElement;
    expect(submit.disabled).toBe(true);

    const user = userEvent.setup();
    await user.type(
      screen.getByTestId("delete-environment-confirm-input"),
      "staging",
    );
    expect(submit.disabled).toBe(false);
    await user.click(submit);
    expect(onConfirm).toHaveBeenCalledWith("staging.toml");
  });

  it("fires onCancel on Cancel click", async () => {
    const onCancel = vi.fn();
    renderWithProviders(
      <DeleteEnvironmentConfirm env={env()} onCancel={onCancel} />,
    );
    await userEvent
      .setup()
      .click(screen.getByTestId("delete-environment-cancel"));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("data-target reflects the env filename for test queries", () => {
    renderWithProviders(<DeleteEnvironmentConfirm env={env()} />);
    expect(
      screen
        .getByTestId("delete-environment-confirm")
        .getAttribute("data-target"),
    ).toBe("staging.toml");
  });

  it("body mentions the secret count when supplied", () => {
    renderWithProviders(
      <DeleteEnvironmentConfirm env={env({ varCount: 5 })} secretCount={2} />,
    );
    const body = screen.getByTestId(
      "delete-environment-confirm-body",
    ).textContent;
    expect(body).toMatch(/5 vars/);
    expect(body).toMatch(/2 secrets/);
  });

  it("Enter inside the confirm input fires onConfirm when matched", async () => {
    const onConfirm = vi.fn();
    renderWithProviders(
      <DeleteEnvironmentConfirm env={env()} onConfirm={onConfirm} />,
    );
    const user = userEvent.setup();
    await user.type(
      screen.getByTestId("delete-environment-confirm-input"),
      "staging{Enter}",
    );
    expect(onConfirm).toHaveBeenCalledWith("staging.toml");
  });

  it("Escape inside the confirm input fires onCancel", async () => {
    const onCancel = vi.fn();
    renderWithProviders(
      <DeleteEnvironmentConfirm env={env()} onCancel={onCancel} />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("delete-environment-confirm-input"));
    await user.keyboard("{Escape}");
    expect(onCancel).toHaveBeenCalled();
  });
});
