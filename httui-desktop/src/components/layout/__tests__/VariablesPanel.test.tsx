import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderWithProviders, screen, waitFor } from "@/test/render";
import userEvent from "@testing-library/user-event";

import { VariablesPanel } from "@/components/layout/VariablesPanel";
import { useEnvironmentStore } from "@/stores/environment";
import type { EnvVariable } from "@/lib/tauri/commands";

const mkEnv = (id: string, name: string) => ({
  id,
  name,
  is_active: true,
  created_at: "2026-01-01T00:00:00Z",
});

const mkVar = (
  key: string,
  value: string,
  isSecret = false,
): EnvVariable => ({
  id: `var-${key}`,
  environment_id: "env-1",
  key,
  value,
  is_secret: isSecret,
  created_at: "2026-01-01T00:00:00Z",
});

beforeEach(() => {
  useEnvironmentStore.setState({
    environments: [],
    activeEnvironment: null,
    managerOpen: false,
    variablesVersion: 0,
    loadVariables: vi.fn(async () => []),
    openManager: vi.fn(),
  } as never);
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("VariablesPanel", () => {
  it('shows "No active environment" when no env is active', () => {
    renderWithProviders(<VariablesPanel />);
    expect(screen.getByText("No active environment")).toBeInTheDocument();
  });

  it('shows "No variables" when the env has none', async () => {
    useEnvironmentStore.setState({
      activeEnvironment: mkEnv("env-1", "local"),
      loadVariables: vi.fn(async () => []),
      variablesVersion: 0,
    } as never);
    renderWithProviders(<VariablesPanel />);
    await waitFor(() =>
      expect(screen.getByText("No variables")).toBeInTheDocument(),
    );
  });

  it("renders each variable with key + value", async () => {
    useEnvironmentStore.setState({
      activeEnvironment: mkEnv("env-1", "local"),
      loadVariables: vi.fn(async () => [
        mkVar("API_URL", "https://api.example.com"),
        mkVar("PORT", "5432"),
      ]),
      variablesVersion: 0,
    } as never);
    renderWithProviders(<VariablesPanel />);
    await waitFor(() =>
      expect(screen.getByText("API_URL")).toBeInTheDocument(),
    );
    expect(screen.getByText("PORT")).toBeInTheDocument();
    expect(screen.getByText("5432")).toBeInTheDocument();
  });

  it("masks secret values and renders a key icon next to them", async () => {
    useEnvironmentStore.setState({
      activeEnvironment: mkEnv("env-1", "local"),
      loadVariables: vi.fn(async () => [
        mkVar("DB_PASSWORD", "supersecret", true),
        mkVar("API_URL", "https://api.example.com"),
      ]),
      variablesVersion: 0,
    } as never);
    renderWithProviders(<VariablesPanel />);
    await waitFor(() =>
      expect(screen.getByText("DB_PASSWORD")).toBeInTheDocument(),
    );
    // Mask shown instead of plain text.
    expect(screen.queryByText("supersecret")).toBeNull();
    expect(screen.getByText("••••••••")).toBeInTheDocument();
    // LuKey icon present for the secret row only.
    expect(
      screen.getByTestId("var-key-icon-DB_PASSWORD"),
    ).toBeInTheDocument();
    expect(
      screen.queryByTestId("var-key-icon-API_URL"),
    ).toBeNull();
  });

  it("clicking the edit button opens the manager", async () => {
    const openManager = vi.fn();
    useEnvironmentStore.setState({
      activeEnvironment: mkEnv("env-1", "local"),
      loadVariables: vi.fn(async () => []),
      openManager,
    } as never);
    const user = userEvent.setup();
    renderWithProviders(<VariablesPanel />);
    await user.click(screen.getByLabelText("Edit variables"));
    expect(openManager).toHaveBeenCalled();
  });

  it("re-fetches when activeEnvironment changes", async () => {
    const loadVariables = vi.fn(async (envId: string) =>
      envId === "env-1"
        ? [mkVar("ONE", "1")]
        : [mkVar("TWO", "2")],
    );
    useEnvironmentStore.setState({
      activeEnvironment: mkEnv("env-1", "local"),
      loadVariables,
      variablesVersion: 0,
    } as never);
    const { rerender } = renderWithProviders(<VariablesPanel />);
    await waitFor(() => expect(screen.getByText("ONE")).toBeInTheDocument());

    useEnvironmentStore.setState({
      activeEnvironment: mkEnv("env-2", "prod"),
    } as never);
    rerender(<VariablesPanel />);
    await waitFor(() => expect(screen.getByText("TWO")).toBeInTheDocument());
  });
});
