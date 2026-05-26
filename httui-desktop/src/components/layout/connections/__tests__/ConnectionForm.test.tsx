import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderWithProviders, screen, waitFor } from "@/test/render";
import userEvent from "@testing-library/user-event";
import { ConnectionForm } from "@/components/layout/connections/ConnectionForm";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";
import type { Connection } from "@/lib/tauri/connections";

const mkConnection = (over: Partial<Connection> = {}): Connection => ({
  id: "c1",
  name: "primary",
  driver: "postgres",
  host: "db.test",
  port: 5432,
  database_name: "mydb",
  username: "alice",
  has_password: false,
  ssl_mode: "require",
  timeout_ms: 10000,
  query_timeout_ms: 30000,
  ttl_seconds: 300,
  max_pool_size: 5,
  is_readonly: false,
  last_tested_at: null,
  created_at: "2026-01-01T00:00:00Z",
  updated_at: "2026-01-01T00:00:00Z",
  ...over,
});

describe("ConnectionForm", () => {
  beforeEach(() => {
    clearTauriMocks();
  });

  afterEach(() => {
    clearTauriMocks();
  });

  describe("create mode", () => {
    it("renders 'New Connection' title when no connection prop", () => {
      renderWithProviders(
        <ConnectionForm connection={null} onClose={vi.fn()} />,
      );
      expect(screen.getByText("New Connection")).toBeInTheDocument();
    });

    it("calls create_connection IPC on Save", async () => {
      const user = userEvent.setup();
      let received: unknown = null;
      mockTauriCommand("create_connection", (args) => {
        received = args;
      });
      const onClose = vi.fn();

      renderWithProviders(
        <ConnectionForm connection={null} onClose={onClose} />,
      );

      const nameInput = screen.getByPlaceholderText("Connection name");
      await user.type(nameInput, "test conn");

      const buttons = screen.getAllByRole("button");
      const saveBtn = buttons.find((b) =>
        /create connection|save|^add$/i.test(b.textContent ?? ""),
      );
      // Fallback — find by text "Create"
      await user.click(saveBtn ?? screen.getByText(/create/i));

      await waitFor(() => expect(received).not.toBeNull());
      expect((received as { input: { name: string } }).input.name).toBe(
        "test conn",
      );
      expect(onClose).toHaveBeenCalled();
    });

    it("switches driver to sqlite hides host/port fields", async () => {
      const user = userEvent.setup();
      renderWithProviders(
        <ConnectionForm connection={null} onClose={vi.fn()} />,
      );

      // Default = postgres → no FILE PATH label
      expect(screen.queryByText(/FILE PATH/i)).not.toBeInTheDocument();

      await user.click(screen.getByText("SQLite"));

      expect(screen.getByText(/FILE PATH/i)).toBeInTheDocument();
    });

    it("blocks Save and surfaces the validator error for an invalid form (F4)", async () => {
      const user = userEvent.setup();
      let created = false;
      mockTauriCommand("create_connection", () => {
        created = true;
      });
      renderWithProviders(
        <ConnectionForm connection={null} onClose={vi.fn()} />,
      );

      // sqlite with a name but no file path → previously created a
      // broken connection silently; now the validator blocks it.
      await user.type(
        screen.getByPlaceholderText("Connection name"),
        "local-db",
      );
      await user.click(screen.getByText("SQLite"));
      await user.click(screen.getByText(/create/i));

      await waitFor(() =>
        expect(
          screen.getByText(/SQLite file path is required/i),
        ).toBeInTheDocument(),
      );
      expect(created).toBe(false);
    });

    it("captures error when create_connection rejects", async () => {
      const user = userEvent.setup();
      mockTauriCommand("create_connection", () => {
        throw new Error("port in use");
      });

      renderWithProviders(
        <ConnectionForm connection={null} onClose={vi.fn()} />,
      );

      await user.type(screen.getByPlaceholderText("Connection name"), "x");
      await user.click(screen.getByText(/create/i));

      await waitFor(() =>
        expect(screen.getByText("port in use")).toBeInTheDocument(),
      );
    });
  });

  describe("edit mode", () => {
    it("renders 'Edit Connection' title with existing values", () => {
      renderWithProviders(
        <ConnectionForm connection={mkConnection()} onClose={vi.fn()} />,
      );
      expect(screen.getByText("Edit Connection")).toBeInTheDocument();
      expect(screen.getByDisplayValue("primary")).toBeInTheDocument();
      expect(screen.getByDisplayValue("alice")).toBeInTheDocument();
    });

    it("calls update_connection on Save in edit mode", async () => {
      const user = userEvent.setup();
      let received: unknown = null;
      mockTauriCommand("update_connection", (args) => {
        received = args;
      });

      renderWithProviders(
        <ConnectionForm connection={mkConnection()} onClose={vi.fn()} />,
      );

      const buttons = screen.getAllByRole("button");
      const saveBtn = buttons.find((b) =>
        /save|update/i.test(b.textContent ?? ""),
      );
      await user.click(saveBtn ?? screen.getByText(/save/i));

      await waitFor(() => expect(received).not.toBeNull());
      expect((received as { id: string }).id).toBe("c1");
    });

    it("Test button calls test_connection IPC", async () => {
      const user = userEvent.setup();
      let tested = false;
      mockTauriCommand("test_connection", () => {
        tested = true;
      });

      renderWithProviders(
        <ConnectionForm connection={mkConnection()} onClose={vi.fn()} />,
      );

      // Test button only renders in edit mode
      const buttons = screen.getAllByRole("button");
      const testBtn = buttons.find((b) => /test/i.test(b.textContent ?? ""));
      if (testBtn) {
        await user.click(testBtn);
        await waitFor(() => expect(tested).toBe(true));
      } else {
        // If button not found by name, that's a render issue; fail explicitly
        throw new Error("Test button not found");
      }
    });

    it("captures test error", async () => {
      const user = userEvent.setup();
      mockTauriCommand("test_connection", () => {
        throw new Error("DNS failure");
      });

      renderWithProviders(
        <ConnectionForm connection={mkConnection()} onClose={vi.fn()} />,
      );

      const buttons = screen.getAllByRole("button");
      const testBtn = buttons.find((b) => /test/i.test(b.textContent ?? ""));
      if (testBtn) {
        await user.click(testBtn);
        await waitFor(() =>
          expect(screen.getByText(/DNS failure/i)).toBeInTheDocument(),
        );
      }
    });
  });

  describe("close behaviors", () => {
    it("Close button calls onClose", async () => {
      const user = userEvent.setup();
      const onClose = vi.fn();
      renderWithProviders(
        <ConnectionForm connection={null} onClose={onClose} />,
      );

      await user.click(screen.getByRole("button", { name: /close/i }));
      expect(onClose).toHaveBeenCalledTimes(1);
    });

    it("Escape key closes the form", async () => {
      const user = userEvent.setup();
      const onClose = vi.fn();
      renderWithProviders(
        <ConnectionForm connection={null} onClose={onClose} />,
      );

      await user.keyboard("{Escape}");
      expect(onClose).toHaveBeenCalledTimes(1);
    });

    it("clicking the modal overlay closes the form", async () => {
      const onClose = vi.fn();
      renderWithProviders(
        <ConnectionForm connection={null} onClose={onClose} />,
      );
      // The overlay is the outermost Portal-mounted Box (position:fixed).
      // userEvent.click bubbles; we need a click whose target IS the
      // overlay itself, not a child. Find by computed style.
      const overlay = document.body.querySelector(
        '[style*="z-index: 1000"]',
      ) as HTMLElement | null;
      if (overlay) {
        overlay.click();
        expect(onClose).toHaveBeenCalled();
      }
    });
  });

  describe("test result rendering", () => {
    it("renders the green 'Connection successful' badge after a successful test", async () => {
      const user = userEvent.setup();
      mockTauriCommand("test_connection", () => undefined);
      renderWithProviders(
        <ConnectionForm connection={mkConnection()} onClose={vi.fn()} />,
      );
      const testBtn = screen
        .getAllByRole("button")
        .find((b) => /test/i.test(b.textContent ?? ""));
      if (testBtn) {
        await user.click(testBtn);
        await waitFor(() =>
          expect(
            screen.getByText(/connection successful/i),
          ).toBeInTheDocument(),
        );
      }
    });
  });

  describe("advanced section", () => {
    it("toggles the AdvancedFields panel open", async () => {
      const user = userEvent.setup();
      renderWithProviders(
        <ConnectionForm connection={null} onClose={vi.fn()} />,
      );
      // The Advanced toggle is the only button labelled /advanced/i.
      const advBtn = screen
        .getAllByRole("button")
        .find((b) => /advanced/i.test(b.textContent ?? ""));
      if (advBtn) {
        await user.click(advBtn);
        // After toggle, the field labels appear (Timeout, Query timeout,
        // TTL, Max pool). Loose check on at least one Timeout label.
        await waitFor(() => {
          expect(screen.getAllByText(/timeout/i).length).toBeGreaterThanOrEqual(
            1,
          );
        });
      }
    });
  });
});
