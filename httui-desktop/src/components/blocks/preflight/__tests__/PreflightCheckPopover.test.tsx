import { describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { PreflightCheckPopover } from "@/components/blocks/preflight/PreflightCheckPopover";
import { renderWithProviders, screen } from "@/test/render";

describe("PreflightCheckPopover (V6 cenário 9 builder)", () => {
  describe("add mode (no initialKind)", () => {
    it("opens at the kind picker stage", () => {
      renderWithProviders(
        <PreflightCheckPopover onSave={vi.fn()} onClose={vi.fn()} />,
      );
      expect(
        screen.getByTestId("preflight-check-popover-kind-picker"),
      ).toBeInTheDocument();
      // No value input yet.
      expect(
        screen.queryByTestId("preflight-check-popover-value"),
      ).not.toBeInTheDocument();
    });

    it("renders all six kind options", () => {
      renderWithProviders(
        <PreflightCheckPopover onSave={vi.fn()} onClose={vi.fn()} />,
      );
      for (const kind of [
        "connection",
        "env_var",
        "branch",
        "keychain",
        "file_exists",
        "command",
      ]) {
        expect(
          screen.getByTestId(`preflight-check-popover-kind-${kind}`),
        ).toBeInTheDocument();
      }
    });

    it("clicking a kind advances to the value input stage", async () => {
      const user = userEvent.setup();
      renderWithProviders(
        <PreflightCheckPopover onSave={vi.fn()} onClose={vi.fn()} />,
      );
      await user.click(
        screen.getByTestId("preflight-check-popover-kind-connection"),
      );
      expect(
        screen.getByTestId("preflight-check-popover-value"),
      ).toBeInTheDocument();
      expect(
        screen.getByTestId("preflight-check-popover-kind").textContent,
      ).toBe("connection");
    });

    it("Save fires onSave with the assembled check", async () => {
      const onSave = vi.fn();
      const user = userEvent.setup();
      renderWithProviders(
        <PreflightCheckPopover onSave={onSave} onClose={vi.fn()} />,
      );
      await user.click(
        screen.getByTestId("preflight-check-popover-kind-command"),
      );
      const input = screen.getByTestId(
        "preflight-check-popover-value",
      ) as HTMLInputElement;
      await user.type(input, "psql");
      await user.click(screen.getByTestId("preflight-check-popover-save"));
      expect(onSave).toHaveBeenCalledWith({ kind: "command", value: "psql" });
    });

    it("Enter in the value input commits", async () => {
      const onSave = vi.fn();
      const user = userEvent.setup();
      renderWithProviders(
        <PreflightCheckPopover onSave={onSave} onClose={vi.fn()} />,
      );
      await user.click(
        screen.getByTestId("preflight-check-popover-kind-env_var"),
      );
      const input = screen.getByTestId("preflight-check-popover-value");
      await user.type(input, "API_TOKEN{Enter}");
      expect(onSave).toHaveBeenCalledWith({
        kind: "env_var",
        value: "API_TOKEN",
      });
    });

    it("trims whitespace before saving", async () => {
      const onSave = vi.fn();
      const user = userEvent.setup();
      renderWithProviders(
        <PreflightCheckPopover onSave={onSave} onClose={vi.fn()} />,
      );
      await user.click(
        screen.getByTestId("preflight-check-popover-kind-command"),
      );
      await user.type(
        screen.getByTestId("preflight-check-popover-value"),
        "  ls  ",
      );
      await user.click(screen.getByTestId("preflight-check-popover-save"));
      expect(onSave).toHaveBeenCalledWith({ kind: "command", value: "ls" });
    });

    it("Save is disabled when value is empty", async () => {
      const onSave = vi.fn();
      const user = userEvent.setup();
      renderWithProviders(
        <PreflightCheckPopover onSave={onSave} onClose={vi.fn()} />,
      );
      await user.click(
        screen.getByTestId("preflight-check-popover-kind-command"),
      );
      // Save button is disabled — clicking it shouldn't fire.
      await user.click(screen.getByTestId("preflight-check-popover-save"));
      expect(onSave).not.toHaveBeenCalled();
    });

    it("Back returns to the kind picker", async () => {
      const user = userEvent.setup();
      renderWithProviders(
        <PreflightCheckPopover onSave={vi.fn()} onClose={vi.fn()} />,
      );
      await user.click(
        screen.getByTestId("preflight-check-popover-kind-connection"),
      );
      await user.click(screen.getByTestId("preflight-check-popover-back"));
      expect(
        screen.getByTestId("preflight-check-popover-kind-picker"),
      ).toBeInTheDocument();
    });
  });

  describe("edit mode (initialKind + initialValue)", () => {
    it("skips the kind picker and lands on the value input", () => {
      renderWithProviders(
        <PreflightCheckPopover
          initialKind="connection"
          initialValue="payments-db"
          onSave={vi.fn()}
          onClose={vi.fn()}
          onRemove={vi.fn()}
        />,
      );
      expect(
        screen.queryByTestId("preflight-check-popover-kind-picker"),
      ).not.toBeInTheDocument();
      const input = screen.getByTestId(
        "preflight-check-popover-value",
      ) as HTMLInputElement;
      expect(input.value).toBe("payments-db");
    });

    it("Remove button fires onRemove", async () => {
      const onRemove = vi.fn();
      const user = userEvent.setup();
      renderWithProviders(
        <PreflightCheckPopover
          initialKind="command"
          initialValue="psql"
          onSave={vi.fn()}
          onClose={vi.fn()}
          onRemove={onRemove}
        />,
      );
      await user.click(screen.getByTestId("preflight-check-popover-remove"));
      expect(onRemove).toHaveBeenCalledTimes(1);
    });

    it("hides the back button (kind is fixed)", () => {
      renderWithProviders(
        <PreflightCheckPopover
          initialKind="branch"
          initialValue="main"
          onSave={vi.fn()}
          onClose={vi.fn()}
          onRemove={vi.fn()}
        />,
      );
      expect(
        screen.queryByTestId("preflight-check-popover-back"),
      ).not.toBeInTheDocument();
    });
  });

  describe("dismiss", () => {
    it("Cancel button closes", async () => {
      const onClose = vi.fn();
      const user = userEvent.setup();
      renderWithProviders(
        <PreflightCheckPopover
          initialKind="command"
          initialValue="ls"
          onSave={vi.fn()}
          onClose={onClose}
          onRemove={vi.fn()}
        />,
      );
      await user.click(screen.getByTestId("preflight-check-popover-cancel"));
      expect(onClose).toHaveBeenCalledTimes(1);
    });

    it("clicking the overlay closes", async () => {
      const onClose = vi.fn();
      const user = userEvent.setup();
      renderWithProviders(
        <PreflightCheckPopover onSave={vi.fn()} onClose={onClose} />,
      );
      await user.click(
        screen.getByTestId("preflight-check-popover-overlay"),
      );
      expect(onClose).toHaveBeenCalledTimes(1);
    });

    it("Escape closes from the kind-picker stage", () => {
      const onClose = vi.fn();
      renderWithProviders(
        <PreflightCheckPopover onSave={vi.fn()} onClose={onClose} />,
      );
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
      expect(onClose).toHaveBeenCalledTimes(1);
    });

    it("Escape closes from the value-input stage", async () => {
      const user = userEvent.setup();
      const onClose = vi.fn();
      renderWithProviders(
        <PreflightCheckPopover onSave={vi.fn()} onClose={onClose} />,
      );
      await user.click(
        screen.getByTestId("preflight-check-popover-kind-command"),
      );
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
      expect(onClose).toHaveBeenCalledTimes(1);
    });
  });
});
