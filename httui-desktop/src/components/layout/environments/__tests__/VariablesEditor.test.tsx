import { describe, it, expect, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";
import { VariablesEditor } from "@/components/layout/environments/VariablesEditor";
import type { EnvVariable } from "@/lib/tauri/commands";

const mkVar = (
  id: string,
  key: string,
  value: string,
  isSecret = false,
): EnvVariable => ({
  id,
  environment_id: "e1",
  key,
  value,
  is_secret: isSecret,
  created_at: "2026-01-01T00:00:00Z",
});

const baseProps = {
  envName: "dev",
  isActive: false,
  variables: [],
  revealedKeys: new Set<string>(),
  onSetActive: vi.fn(),
  onDuplicate: vi.fn(),
  onDelete: vi.fn(),
  onSetVariable: vi.fn(async () => {}),
  onDeleteVariable: vi.fn(async () => {}),
  onToggleReveal: vi.fn(),
};

describe("VariablesEditor", () => {
  it("renders the env name and 'Set active' button when not active", () => {
    renderWithProviders(<VariablesEditor {...baseProps} />);
    expect(screen.getByText("dev")).toBeInTheDocument();
    expect(screen.getByText("Set active")).toBeInTheDocument();
  });

  it("renders 'active' badge when env is active (no Set active button)", () => {
    renderWithProviders(<VariablesEditor {...baseProps} isActive={true} />);
    expect(screen.getByText("active")).toBeInTheDocument();
    expect(screen.queryByText("Set active")).not.toBeInTheDocument();
  });

  it("clicking 'Set active' calls onSetActive", async () => {
    const user = userEvent.setup();
    const onSetActive = vi.fn();
    renderWithProviders(
      <VariablesEditor {...baseProps} onSetActive={onSetActive} />,
    );

    await user.click(screen.getByText("Set active"));
    expect(onSetActive).toHaveBeenCalledTimes(1);
  });

  it("Duplicate button triggers onDuplicate", async () => {
    const user = userEvent.setup();
    const onDuplicate = vi.fn();
    renderWithProviders(
      <VariablesEditor {...baseProps} onDuplicate={onDuplicate} />,
    );

    await user.click(screen.getByRole("button", { name: /duplicate/i }));
    expect(onDuplicate).toHaveBeenCalledTimes(1);
  });

  it("Delete button triggers onDelete", async () => {
    const user = userEvent.setup();
    const onDelete = vi.fn();
    renderWithProviders(<VariablesEditor {...baseProps} onDelete={onDelete} />);

    // Two delete-ish buttons may exist (env-level + per-variable). Filter env one
    // by aria-label "Delete" (without "variable").
    const deleteBtns = screen
      .getAllByRole("button")
      .filter((b) => b.getAttribute("aria-label")?.toLowerCase() === "delete");
    expect(deleteBtns).toHaveLength(1);
    await user.click(deleteBtns[0]);
    expect(onDelete).toHaveBeenCalledTimes(1);
  });

  it("renders one VariableRow per variable", () => {
    renderWithProviders(
      <VariablesEditor
        {...baseProps}
        variables={[
          mkVar("v1", "TOKEN", "abc"),
          mkVar("v2", "URL", "https://x"),
        ]}
      />,
    );
    expect(screen.getByText("TOKEN")).toBeInTheDocument();
    expect(screen.getByText("URL")).toBeInTheDocument();
    expect(screen.getByText("abc")).toBeInTheDocument();
    expect(screen.getByText("https://x")).toBeInTheDocument();
  });

  it("KeyValueAddRow at the bottom triggers onSetVariable on submit", async () => {
    const user = userEvent.setup();
    const onSetVariable = vi.fn(async () => {});
    renderWithProviders(
      <VariablesEditor {...baseProps} onSetVariable={onSetVariable} />,
    );

    await user.type(screen.getByPlaceholderText("KEY"), "NEW_KEY");
    await user.type(screen.getByPlaceholderText("value"), "new-val");
    await user.click(screen.getByRole("button", { name: /add/i }));

    expect(onSetVariable).toHaveBeenCalledWith("NEW_KEY", "new-val");
  });

  it("renders the {{KEY}} usage hint", () => {
    renderWithProviders(<VariablesEditor {...baseProps} />);
    expect(screen.getByText(/in HTTP blocks/i)).toBeInTheDocument();
  });

  it("row delete dispatches onDeleteVariable with the row id", async () => {
    const user = userEvent.setup();
    const onDeleteVariable = vi.fn(async () => {});
    renderWithProviders(
      <VariablesEditor
        {...baseProps}
        variables={[mkVar("v1", "TOKEN", "abc")]}
        onDeleteVariable={onDeleteVariable}
      />,
    );

    await user.click(screen.getByLabelText("Delete variable"));
    expect(onDeleteVariable).toHaveBeenCalledWith("v1");
  });

  it("row toggle reveal dispatches onToggleReveal with the row id", async () => {
    const user = userEvent.setup();
    const onToggleReveal = vi.fn();
    renderWithProviders(
      <VariablesEditor
        {...baseProps}
        variables={[mkVar("v1", "TOKEN", "", true)]}
        onToggleReveal={onToggleReveal}
      />,
    );

    await user.click(screen.getByLabelText("Show value"));
    expect(onToggleReveal).toHaveBeenCalledWith("v1");
  });

  it("revealed secret row shows the resolvedValue (not •••)", () => {
    renderWithProviders(
      <VariablesEditor
        {...baseProps}
        variables={[mkVar("v1", "API_KEY", "", true)]}
        revealedKeys={new Set(["v1"])}
        resolvedValues={{ API_KEY: "sk-real-secret" }}
      />,
    );
    expect(screen.getByText("sk-real-secret")).toBeInTheDocument();
    expect(screen.queryByText("••••••••")).toBeNull();
  });
});
