import { describe, it, expect, beforeEach, vi } from "vitest";
import { act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { EnvSwitcher } from "@/components/layout/EnvSwitcher";
import { useEnvironmentStore } from "@/stores/environment";
import { useEnvSwitcherStore } from "@/stores/envSwitcher";
import { renderWithProviders, screen } from "@/test/render";

const mkEnv = (id: string, name: string, is_active = false) => ({
  id,
  name,
  is_active,
  created_at: "2026-01-01T00:00:00Z",
});

const duplicateEnvironment = vi.fn(async () => {});
const switchEnvironment = vi.fn(async () => {});

beforeEach(() => {
  duplicateEnvironment.mockClear();
  switchEnvironment.mockClear();
  useEnvSwitcherStore.setState({ open: false });
  useEnvironmentStore.setState({
    environments: [mkEnv("a", "local", true), mkEnv("b", "staging")],
    activeEnvironment: mkEnv("a", "local", true),
    switchEnvironment,
    duplicateEnvironment,
  } as never);
});

describe("EnvSwitcher", () => {
  it("renders the status-env trigger from the store", () => {
    renderWithProviders(<EnvSwitcher />);
    expect(screen.getByTestId("status-env").textContent).toContain("local");
  });

  it("opens the dropdown when the env-switcher store flips open", async () => {
    renderWithProviders(<EnvSwitcher />);
    act(() => {
      useEnvSwitcherStore.getState().openSwitcher();
    });
    expect(await screen.findByTestId("env-menu")).toBeInTheDocument();
  });

  it("Clone footer opens the clone popover with the active env", async () => {
    const user = userEvent.setup();
    renderWithProviders(<EnvSwitcher />);
    await user.click(screen.getByTestId("status-env"));
    await user.click(await screen.findByTestId("env-menu-clone"));
    const form = await screen.findByTestId("clone-environment-form");
    expect(form.getAttribute("data-source")).toBe("local.toml");
    // Menu closed itself when clone opened.
    expect(useEnvSwitcherStore.getState().open).toBe(false);
  });

  it("submitting the clone form calls duplicateEnvironment", async () => {
    const user = userEvent.setup();
    renderWithProviders(<EnvSwitcher />);
    await user.click(screen.getByTestId("status-env"));
    await user.click(await screen.findByTestId("env-menu-clone"));
    await screen.findByTestId("clone-environment-form");
    await user.type(screen.getByTestId("clone-environment-name"), "local-copy");
    await user.click(screen.getByTestId("clone-environment-save"));
    expect(duplicateEnvironment).toHaveBeenCalledWith("a", "local-copy");
  });

  it("switching an env from the dropdown calls switchEnvironment", async () => {
    const user = userEvent.setup();
    renderWithProviders(<EnvSwitcher />);
    await user.click(screen.getByTestId("status-env"));
    await user.click(await screen.findByText("staging"));
    expect(switchEnvironment).toHaveBeenCalledWith("b");
  });
});
