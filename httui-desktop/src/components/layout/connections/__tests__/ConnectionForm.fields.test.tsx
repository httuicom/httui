// Coverage backfill for ConnectionForm — drives every per-field
// dispatch arrow (uncovered lines 199-277 per v8 report). Each Input /
// Select / toggle callback gets exercised here so the inline arrows
// run during render. The existing `ConnectionForm.test.tsx` covers
// title, Save, driver-switch, validator errors, edit-mode, Test
// button, close behaviors, Advanced toggle smoke.
//
// Coverage gate alvo: ConnectionForm 72% → ≥80%.

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderWithProviders, screen, waitFor } from "@/test/render";
import userEvent from "@testing-library/user-event";
import { ConnectionForm } from "@/components/layout/connections/ConnectionForm";
import { clearTauriMocks, mockTauriCommand } from "@/test/mocks/tauri";

describe("ConnectionForm — per-field dispatch coverage", () => {
  beforeEach(() => clearTauriMocks());
  afterEach(() => clearTauriMocks());

  it("types into all NetworkFields inputs (host/port/dbName/username/password)", async () => {
    const user = userEvent.setup();
    renderWithProviders(<ConnectionForm connection={null} onClose={vi.fn()} />);
    await user.type(
      screen.getByPlaceholderText("Connection name"),
      "interactive",
    );

    // Default driver = postgres → NetworkFields path. Drive every input
    // to exercise the per-field `(value) => dispatch(...)` arrows in
    // ConnectionForm.tsx (uncovered branches per v8 report).
    const inputs = screen.getAllByRole("textbox") as HTMLInputElement[];
    for (const [i, input] of inputs.entries()) {
      if (i === 0) continue; // name already filled
      await user.click(input);
      await user.type(input, "x");
    }

    // Port = NativeSelect or number input. Drive every spinbutton too.
    const spin = screen.queryAllByRole("spinbutton") as HTMLInputElement[];
    for (const s of spin) {
      await user.click(s);
      await user.type(s, "1");
    }

    // sslMode is a NativeSelect — drive the combobox onChange arrow.
    const comboboxes = screen.queryAllByRole("combobox") as HTMLSelectElement[];
    for (const sel of comboboxes) {
      const options = Array.from(sel.options).filter(
        (o) => o.value && o.value !== sel.value,
      );
      if (options.length > 0) {
        await user.selectOptions(sel, options[0].value);
      }
    }

    // Verify the values landed in the form state (each typed char ran
    // the dispatch arrow). Spot-check: at least one input retains "x".
    expect(inputs.some((i) => (i.value ?? "").includes("x"))).toBe(true);
  });

  it("types into SqliteFields dbName input (sqlite driver path)", async () => {
    const user = userEvent.setup();
    let received: { input?: Record<string, unknown> } | null = null;
    mockTauriCommand("create_connection", (args) => {
      received = args as { input: Record<string, unknown> };
    });

    renderWithProviders(<ConnectionForm connection={null} onClose={vi.fn()} />);

    await user.type(screen.getByPlaceholderText("Connection name"), "sqlite1");
    await user.click(screen.getByText("SQLite"));

    // The sqlite dbName input is the file-path field. Type to drive the
    // `dispatch({type:"setField", field:"dbName", ...})` arrow.
    const inputs = screen.getAllByRole("textbox") as HTMLInputElement[];
    // 0 = name, 1+ = sqlite path
    for (const [i, input] of inputs.entries()) {
      if (i === 0) continue;
      await user.click(input);
      await user.type(input, "/tmp/db.sqlite");
    }

    await user.click(screen.getByText(/create/i));
    await waitFor(() => expect(received).not.toBeNull());
  });

  it("toggles AdvancedFields and types into timeout / TTL / pool inputs", async () => {
    const user = userEvent.setup();
    renderWithProviders(<ConnectionForm connection={null} onClose={vi.fn()} />);

    // The Advanced toggle is a <Flex onClick> with data-testid (not a
    // <button>) — query by testid.
    const advToggle = await screen.findByTestId("advanced-toggle");
    await user.click(advToggle);

    // The 4 advanced fields are <Input> (Chakra text inputs), each with
    // a unique placeholder. Find them by placeholder + drive onChange.
    const timeout = await screen.findByPlaceholderText("10000");
    const queryTimeout = await screen.findByPlaceholderText("30000");
    const ttl = await screen.findByPlaceholderText("300");
    const maxPool = await screen.findByPlaceholderText("5");
    for (const input of [
      timeout,
      queryTimeout,
      ttl,
      maxPool,
    ] as HTMLInputElement[]) {
      await user.click(input);
      await user.clear(input);
      await user.type(input, "100");
    }
    expect((timeout as HTMLInputElement).value).toBe("100");
  });

  it("captures test error from non-Error throw (string error path)", async () => {
    // testFailure branch: `err instanceof Error ? err.message : String(err)`.
    // Previous test covered the Error path; this covers the String() fallback.
    const user = userEvent.setup();
    mockTauriCommand("test_connection", () => {
      throw "raw string error";
    });

    renderWithProviders(
      <ConnectionForm
        connection={{
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
          created_at: "",
          updated_at: "",
        }}
        onClose={vi.fn()}
      />,
    );

    const buttons = screen.getAllByRole("button");
    const testBtn = buttons.find((b) => /test/i.test(b.textContent ?? ""));
    if (!testBtn) throw new Error("Test button not found");
    await user.click(testBtn);
    await waitFor(() =>
      expect(screen.getByText(/raw string error/i)).toBeInTheDocument(),
    );
  });

  it("captures save error from non-Error throw (string fallback)", async () => {
    const user = userEvent.setup();
    mockTauriCommand("create_connection", () => {
      throw "raw save error";
    });

    renderWithProviders(<ConnectionForm connection={null} onClose={vi.fn()} />);
    await user.type(screen.getByPlaceholderText("Connection name"), "x");
    await user.click(screen.getByText(/create/i));

    await waitFor(() =>
      expect(screen.getByText(/raw save error/i)).toBeInTheDocument(),
    );
  });
});
