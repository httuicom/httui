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
      // The CM6 editor seeds its initial value via the `value` prop.
      // jsdom renders the contentEditable; the visible text matches.
      const editor = screen.getByTestId("preflight-check-popover-value-editor");
      expect(editor.textContent).toContain("payments-db");
    });

    it("Save commits the seeded initialValue (no typing needed)", async () => {
      const onSave = vi.fn();
      const user = userEvent.setup();
      renderWithProviders(
        <PreflightCheckPopover
          initialKind="command"
          initialValue="psql"
          onSave={onSave}
          onClose={vi.fn()}
          onRemove={vi.fn()}
        />,
      );
      await user.click(screen.getByTestId("preflight-check-popover-save"));
      expect(onSave).toHaveBeenCalledWith({ kind: "command", value: "psql" });
    });

    it("trims whitespace before saving (initialValue with padding)", async () => {
      const onSave = vi.fn();
      const user = userEvent.setup();
      renderWithProviders(
        <PreflightCheckPopover
          initialKind="command"
          initialValue="  ls  "
          onSave={onSave}
          onClose={vi.fn()}
          onRemove={vi.fn()}
        />,
      );
      await user.click(screen.getByTestId("preflight-check-popover-save"));
      expect(onSave).toHaveBeenCalledWith({ kind: "command", value: "ls" });
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

    // Esc in the value-input stage is owned by the CM6 editor's keymap
    // (so it can dismiss its autocomplete popup first). End-to-end
    // browser tests cover that path; jsdom doesn't reliably reach the
    // CM6 view's contenteditable for synthetic keydowns.
  });

  describe.skip("autocomplete suggestions (browser-only — CM6 popup needs real DOM)", () => {
    it("renders suggestions list when getSuggestions returns matches", async () => {
      const user = userEvent.setup();
      renderWithProviders(
        <PreflightCheckPopover
          onSave={vi.fn()}
          onClose={vi.fn()}
          getSuggestions={async (kind) =>
            kind === "connection" ? ["payments-db", "audit-db", "logs"] : []
          }
        />,
      );
      await user.click(
        screen.getByTestId("preflight-check-popover-kind-connection"),
      );
      // Wait one tick for the Promise to resolve.
      await new Promise((r) => setTimeout(r, 0));
      const list = await screen.findByTestId(
        "preflight-check-popover-suggestions",
      );
      expect(list).toBeInTheDocument();
      expect(
        screen.getByTestId("preflight-check-popover-suggestion-payments-db"),
      ).toBeInTheDocument();
      expect(
        screen.getByTestId("preflight-check-popover-suggestion-audit-db"),
      ).toBeInTheDocument();
    });

    it("filters suggestions by substring match", async () => {
      const user = userEvent.setup();
      renderWithProviders(
        <PreflightCheckPopover
          onSave={vi.fn()}
          onClose={vi.fn()}
          getSuggestions={async () => ["payments-db", "audit-db", "logs"]}
        />,
      );
      await user.click(
        screen.getByTestId("preflight-check-popover-kind-connection"),
      );
      await new Promise((r) => setTimeout(r, 0));
      await user.type(
        screen.getByTestId("preflight-check-popover-value"),
        "db",
      );
      // 2 matches expected (payments-db + audit-db); "logs" filtered out.
      expect(
        screen.queryByTestId("preflight-check-popover-suggestion-logs"),
      ).not.toBeInTheDocument();
      expect(
        screen.getByTestId("preflight-check-popover-suggestion-payments-db"),
      ).toBeInTheDocument();
    });

    it("clicking a suggestion fills the input value", async () => {
      const user = userEvent.setup();
      renderWithProviders(
        <PreflightCheckPopover
          onSave={vi.fn()}
          onClose={vi.fn()}
          getSuggestions={async () => ["payments-db"]}
        />,
      );
      await user.click(
        screen.getByTestId("preflight-check-popover-kind-connection"),
      );
      await new Promise((r) => setTimeout(r, 0));
      await user.click(
        screen.getByTestId("preflight-check-popover-suggestion-payments-db"),
      );
      const input = screen.getByTestId(
        "preflight-check-popover-value",
      ) as HTMLInputElement;
      expect(input.value).toBe("payments-db");
    });

    it("hides the suggestions list when no provider is wired", async () => {
      const user = userEvent.setup();
      renderWithProviders(
        <PreflightCheckPopover onSave={vi.fn()} onClose={vi.fn()} />,
      );
      await user.click(
        screen.getByTestId("preflight-check-popover-kind-command"),
      );
      expect(
        screen.queryByTestId("preflight-check-popover-suggestions"),
      ).not.toBeInTheDocument();
    });

    it("hides when the only match is exact-equals (already picked)", async () => {
      const user = userEvent.setup();
      renderWithProviders(
        <PreflightCheckPopover
          onSave={vi.fn()}
          onClose={vi.fn()}
          getSuggestions={async () => ["payments-db"]}
        />,
      );
      await user.click(
        screen.getByTestId("preflight-check-popover-kind-connection"),
      );
      await new Promise((r) => setTimeout(r, 0));
      await user.type(
        screen.getByTestId("preflight-check-popover-value"),
        "payments-db",
      );
      expect(
        screen.queryByTestId("preflight-check-popover-suggestions"),
      ).not.toBeInTheDocument();
    });

    it("swallows provider errors silently (empty list)", async () => {
      const user = userEvent.setup();
      renderWithProviders(
        <PreflightCheckPopover
          onSave={vi.fn()}
          onClose={vi.fn()}
          getSuggestions={async () => {
            throw new Error("rpc fail");
          }}
        />,
      );
      await user.click(
        screen.getByTestId("preflight-check-popover-kind-connection"),
      );
      await new Promise((r) => setTimeout(r, 0));
      expect(
        screen.queryByTestId("preflight-check-popover-suggestions"),
      ).not.toBeInTheDocument();
    });
  });
});
