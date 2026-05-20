import { describe, it, expect, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";
import { StandaloneBlockShell } from "@/components/blocks/StandaloneBlockShell";
import type {
  DisplayMode,
  ExecutionState,
} from "@/components/blocks/ExecutableBlock";

const baseProps = {
  blockType: "http",
  alias: "req1",
  displayMode: "split" as DisplayMode,
  executionState: "idle" as ExecutionState,
  onAliasChange: vi.fn(),
  onDisplayModeChange: vi.fn(),
  onRun: vi.fn(),
  onCancel: vi.fn(),
  inputSlot: <div data-testid="input-slot">INPUT</div>,
  outputSlot: <div data-testid="output-slot">OUTPUT</div>,
};

describe("StandaloneBlockShell", () => {
  describe("header", () => {
    it("shows the block type label (HTTP)", () => {
      renderWithProviders(<StandaloneBlockShell {...baseProps} />);
      expect(screen.getByText("HTTP")).toBeInTheDocument();
    });

    it("falls back to uppercased block type for unknown types", () => {
      renderWithProviders(
        <StandaloneBlockShell {...baseProps} blockType="custom" />,
      );
      expect(screen.getByText("CUSTOM")).toBeInTheDocument();
    });

    it("uses DB label for db block type", () => {
      renderWithProviders(
        <StandaloneBlockShell {...baseProps} blockType="db" />,
      );
      expect(screen.getByText("DB")).toBeInTheDocument();
    });

    it("renders alias input with the current value", () => {
      renderWithProviders(<StandaloneBlockShell {...baseProps} />);
      expect(screen.getByPlaceholderText("alias...")).toHaveValue("req1");
    });

    it("calls onAliasChange when alias is edited", async () => {
      const user = userEvent.setup();
      const onAliasChange = vi.fn();
      renderWithProviders(
        <StandaloneBlockShell
          {...baseProps}
          alias=""
          onAliasChange={onAliasChange}
        />,
      );
      await user.type(screen.getByPlaceholderText("alias..."), "x");
      expect(onAliasChange).toHaveBeenCalled();
    });
  });

  describe("display modes", () => {
    it("calls onDisplayModeChange when clicking Input mode button", async () => {
      const user = userEvent.setup();
      const onDisplayModeChange = vi.fn();
      renderWithProviders(
        <StandaloneBlockShell
          {...baseProps}
          onDisplayModeChange={onDisplayModeChange}
        />,
      );
      await user.click(screen.getByRole("button", { name: "Input" }));
      expect(onDisplayModeChange).toHaveBeenCalledWith("input");
    });

    it("calls onDisplayModeChange when clicking Output mode button", async () => {
      const user = userEvent.setup();
      const onDisplayModeChange = vi.fn();
      renderWithProviders(
        <StandaloneBlockShell
          {...baseProps}
          onDisplayModeChange={onDisplayModeChange}
        />,
      );
      await user.click(screen.getByRole("button", { name: "Output" }));
      expect(onDisplayModeChange).toHaveBeenCalledWith("output");
    });

    it("renders both slots in split mode", () => {
      renderWithProviders(
        <StandaloneBlockShell {...baseProps} displayMode="split" />,
      );
      expect(screen.getByTestId("input-slot")).toBeInTheDocument();
      // outputSlot rendered only when state !== idle, but DOM still mounts the input
    });

    it("renders 'Run to see results' placeholder in idle state", () => {
      renderWithProviders(<StandaloneBlockShell {...baseProps} />);
      expect(screen.getByText("Run to see results")).toBeInTheDocument();
    });

    it("renders the actual outputSlot when not idle", () => {
      renderWithProviders(
        <StandaloneBlockShell {...baseProps} executionState="success" />,
      );
      expect(screen.getByTestId("output-slot")).toBeInTheDocument();
      expect(screen.queryByText("Run to see results")).not.toBeInTheDocument();
    });
  });

  describe("run / cancel", () => {
    it("clicking the action button in idle state calls onRun", async () => {
      const user = userEvent.setup();
      const onRun = vi.fn();
      const onCancel = vi.fn();
      renderWithProviders(
        <StandaloneBlockShell
          {...baseProps}
          onRun={onRun}
          onCancel={onCancel}
        />,
      );

      await user.click(screen.getByRole("button", { name: "Run" }));
      expect(onRun).toHaveBeenCalledTimes(1);
      expect(onCancel).not.toHaveBeenCalled();
    });

    it("clicking the action button while running calls onCancel", async () => {
      const user = userEvent.setup();
      const onRun = vi.fn();
      const onCancel = vi.fn();
      renderWithProviders(
        <StandaloneBlockShell
          {...baseProps}
          executionState="running"
          onRun={onRun}
          onCancel={onCancel}
        />,
      );

      await user.click(screen.getByRole("button", { name: "Cancel" }));
      expect(onCancel).toHaveBeenCalledTimes(1);
      expect(onRun).not.toHaveBeenCalled();
    });

    it("shows custom statusText when running", () => {
      renderWithProviders(
        <StandaloneBlockShell
          {...baseProps}
          executionState="running"
          statusText="downloading 1.2 MB…"
        />,
      );
      expect(screen.getByText(/downloading 1\.2 MB/)).toBeInTheDocument();
    });

    it("shows state label when not running", () => {
      renderWithProviders(
        <StandaloneBlockShell {...baseProps} executionState="success" />,
      );
      expect(screen.getByText("success")).toBeInTheDocument();
    });

    it("shows 'cached' state label", () => {
      renderWithProviders(
        <StandaloneBlockShell {...baseProps} executionState="cached" />,
      );
      expect(screen.getByText("cached")).toBeInTheDocument();
    });
  });

  describe("delete button", () => {
    it("does not render when onDelete is not provided", () => {
      renderWithProviders(<StandaloneBlockShell {...baseProps} />);
      expect(
        screen.queryByRole("button", { name: /delete block/i }),
      ).not.toBeInTheDocument();
    });

    it("renders and triggers callback when onDelete is provided", async () => {
      const user = userEvent.setup();
      const onDelete = vi.fn();
      renderWithProviders(
        <StandaloneBlockShell {...baseProps} onDelete={onDelete} />,
      );

      await user.click(screen.getByRole("button", { name: /delete block/i }));
      expect(onDelete).toHaveBeenCalledTimes(1);
    });
  });
});
