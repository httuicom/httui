import { describe, it, expect, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";
import { HttpToolbar } from "@/components/blocks/http/fenced/HttpToolbar";
import type { HttpMethod } from "@/lib/blocks/http-message";
import type { HttpBodyMode } from "@/lib/blocks/http-body-modes";
import type { ExecutionState } from "@/components/blocks/http/fenced/shared";

const baseProps = {
  alias: undefined as string | undefined,
  method: "GET" as HttpMethod,
  host: null as string | null,
  mode: "raw" as "raw" | "form",
  bodyMode: "json" as HttpBodyMode,
  executionState: "idle" as ExecutionState,
  onRun: vi.fn(),
  onCancel: vi.fn(),
  onOpenSettings: vi.fn(),
  onToggleMode: vi.fn(),
  onPickBodyMode: vi.fn(),
};

describe("HttpToolbar", () => {
  describe("rendering", () => {
    it("always renders the HTTP badge and method", () => {
      renderWithProviders(<HttpToolbar {...baseProps} method="POST" />);
      expect(screen.getByText("HTTP")).toBeInTheDocument();
      expect(screen.getByText("POST")).toBeInTheDocument();
    });

    it("does NOT render the alias slot when alias is undefined", () => {
      renderWithProviders(<HttpToolbar {...baseProps} alias={undefined} />);
      expect(screen.queryByLabelText("alias")).not.toBeInTheDocument();
    });

    it("renders the alias slot when alias is provided", () => {
      renderWithProviders(<HttpToolbar {...baseProps} alias="createUser" />);
      const aliasEl = screen.getByLabelText("alias");
      expect(aliasEl).toHaveTextContent("createUser");
    });

    it("does NOT render host text when host is null", () => {
      renderWithProviders(<HttpToolbar {...baseProps} host={null} />);
      expect(screen.queryByText(/api\.example/)).not.toBeInTheDocument();
    });

    it("renders host text when present", () => {
      renderWithProviders(
        <HttpToolbar {...baseProps} host="api.example.com" />,
      );
      expect(screen.getByText("api.example.com")).toBeInTheDocument();
    });
  });

  describe("mode toggle (raw / form)", () => {
    it("highlights 'raw' when mode='raw' and 'form' when mode='form'", () => {
      const { rerender } = renderWithProviders(
        <HttpToolbar {...baseProps} mode="raw" />,
      );
      expect(screen.getByRole("button", { name: "raw" })).toHaveAttribute(
        "aria-pressed",
        "true",
      );
      expect(screen.getByRole("button", { name: "form" })).toHaveAttribute(
        "aria-pressed",
        "false",
      );

      rerender(<HttpToolbar {...baseProps} mode="form" />);
      expect(screen.getByRole("button", { name: "form" })).toHaveAttribute(
        "aria-pressed",
        "true",
      );
    });

    it("clicking 'form' calls onToggleMode('form')", async () => {
      const user = userEvent.setup();
      const onToggleMode = vi.fn();
      renderWithProviders(
        <HttpToolbar {...baseProps} onToggleMode={onToggleMode} />,
      );
      await user.click(screen.getByRole("button", { name: "form" }));
      expect(onToggleMode).toHaveBeenCalledWith("form");
    });

    it("clicking 'raw' calls onToggleMode('raw')", async () => {
      const user = userEvent.setup();
      const onToggleMode = vi.fn();
      renderWithProviders(
        <HttpToolbar {...baseProps} mode="form" onToggleMode={onToggleMode} />,
      );
      await user.click(screen.getByRole("button", { name: "raw" }));
      expect(onToggleMode).toHaveBeenCalledWith("raw");
    });
  });

  describe("body-mode menu", () => {
    it("renders current bodyMode as the trigger label", () => {
      renderWithProviders(<HttpToolbar {...baseProps} bodyMode="multipart" />);
      expect(
        screen.getByRole("button", { name: "Body mode: multipart" }),
      ).toHaveTextContent("multipart");
    });
  });

  describe("run / cancel", () => {
    it("idle state renders Run button (not Cancel)", () => {
      renderWithProviders(<HttpToolbar {...baseProps} executionState="idle" />);
      expect(
        screen.getByRole("button", { name: /run request/i }),
      ).toBeInTheDocument();
      expect(
        screen.queryByRole("button", { name: /cancel request/i }),
      ).not.toBeInTheDocument();
    });

    it("running state renders Cancel button (not Run)", () => {
      renderWithProviders(
        <HttpToolbar {...baseProps} executionState="running" />,
      );
      expect(
        screen.getByRole("button", { name: /cancel request/i }),
      ).toBeInTheDocument();
      expect(
        screen.queryByRole("button", { name: /run request/i }),
      ).not.toBeInTheDocument();
    });

    it("clicking Run calls onRun (not onCancel)", async () => {
      const user = userEvent.setup();
      const onRun = vi.fn();
      const onCancel = vi.fn();
      renderWithProviders(
        <HttpToolbar
          {...baseProps}
          onRun={onRun}
          onCancel={onCancel}
          executionState="idle"
        />,
      );
      await user.click(screen.getByRole("button", { name: /run request/i }));
      expect(onRun).toHaveBeenCalledTimes(1);
      expect(onCancel).not.toHaveBeenCalled();
    });

    it("clicking Cancel calls onCancel (not onRun)", async () => {
      const user = userEvent.setup();
      const onRun = vi.fn();
      const onCancel = vi.fn();
      renderWithProviders(
        <HttpToolbar
          {...baseProps}
          onRun={onRun}
          onCancel={onCancel}
          executionState="running"
        />,
      );
      await user.click(screen.getByRole("button", { name: /cancel request/i }));
      expect(onCancel).toHaveBeenCalledTimes(1);
      expect(onRun).not.toHaveBeenCalled();
    });
  });

  describe("settings", () => {
    it("clicking Settings calls onOpenSettings", async () => {
      const user = userEvent.setup();
      const onOpenSettings = vi.fn();
      renderWithProviders(
        <HttpToolbar {...baseProps} onOpenSettings={onOpenSettings} />,
      );
      await user.click(screen.getByRole("button", { name: /block settings/i }));
      expect(onOpenSettings).toHaveBeenCalledTimes(1);
    });
  });
});
