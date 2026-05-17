import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";

import { SegmentedEnvSwitcher } from "@/components/layout/topbar/SegmentedEnvSwitcher";
import { useEnvironmentStore } from "@/stores/environment";
import { clearTauriMocks } from "@/test/mocks/tauri";

const mkEnv = (id: string, name: string) => ({
  id,
  name,
  is_active: false,
  created_at: "2026-01-01T00:00:00Z",
});

const switchSpy = vi.fn();

beforeEach(() => {
  switchSpy.mockClear();
  clearTauriMocks();
  useEnvironmentStore.setState({
    environments: [],
    activeEnvironment: null,
    managerOpen: false,
    variablesVersion: 0,
    switchEnvironment: switchSpy,
  } as never);
});

afterEach(() => {
  clearTauriMocks();
});

describe("SegmentedEnvSwitcher", () => {
  it("shows 'no env' fallback when no environments exist", () => {
    renderWithProviders(<SegmentedEnvSwitcher />);
    expect(screen.getByText("no env")).toBeInTheDocument();
  });

  it("renders one role=tab per environment", () => {
    useEnvironmentStore.setState({
      environments: [
        mkEnv("a", "local"),
        mkEnv("b", "staging"),
        mkEnv("c", "prod"),
      ],
      activeEnvironment: mkEnv("a", "local"),
      switchEnvironment: switchSpy,
    } as never);

    renderWithProviders(<SegmentedEnvSwitcher />);
    expect(screen.getAllByRole("tab")).toHaveLength(3);
    expect(screen.getByText("local")).toBeInTheDocument();
    expect(screen.getByText("staging")).toBeInTheDocument();
    expect(screen.getByText("prod")).toBeInTheDocument();
  });

  it("marks the active env with aria-selected + data-active='true'", () => {
    useEnvironmentStore.setState({
      environments: [mkEnv("a", "local"), mkEnv("b", "staging")],
      activeEnvironment: mkEnv("b", "staging"),
      switchEnvironment: switchSpy,
    } as never);

    renderWithProviders(<SegmentedEnvSwitcher />);
    const staging = screen.getByRole("tab", { name: /staging/ });
    expect(staging.getAttribute("aria-selected")).toBe("true");
    expect(staging.getAttribute("data-active")).toBe("true");

    const local = screen.getByRole("tab", { name: /local/ });
    expect(local.getAttribute("aria-selected")).toBe("false");
  });

  it("clicking a non-active cell calls switchEnvironment with its id", async () => {
    useEnvironmentStore.setState({
      environments: [mkEnv("a", "local"), mkEnv("b", "prod")],
      activeEnvironment: mkEnv("a", "local"),
      switchEnvironment: switchSpy,
    } as never);

    renderWithProviders(<SegmentedEnvSwitcher />);
    await userEvent.setup().click(screen.getByRole("tab", { name: /prod/ }));
    expect(switchSpy).toHaveBeenCalledWith("b");
  });

  it("clicking the active cell does NOT re-dispatch switchEnvironment", async () => {
    useEnvironmentStore.setState({
      environments: [mkEnv("a", "local")],
      activeEnvironment: mkEnv("a", "local"),
      switchEnvironment: switchSpy,
    } as never);

    renderWithProviders(<SegmentedEnvSwitcher />);
    await userEvent.setup().click(screen.getByRole("tab", { name: /local/ }));
    expect(switchSpy).not.toHaveBeenCalled();
  });

  it("renders a Dot variant=err on environments whose name starts with 'prod'", () => {
    useEnvironmentStore.setState({
      environments: [
        mkEnv("a", "local"),
        mkEnv("b", "prod"),
        mkEnv("c", "prod-canary"),
      ],
      activeEnvironment: mkEnv("a", "local"),
      switchEnvironment: switchSpy,
    } as never);

    const { container } = renderWithProviders(<SegmentedEnvSwitcher />);
    // 2 prod-prefixed envs → 2 err dots inside the prod cells
    const errDots = container.querySelectorAll(
      '[data-atom="dot"][data-variant="err"]',
    );
    expect(errDots.length).toBe(2);
  });
});
