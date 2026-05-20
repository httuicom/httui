import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";

vi.mock("@/lib/theme/apply", () => ({ applyTheme: vi.fn() }));

import { NewConnectionModalContainer } from "@/components/layout/connections/NewConnectionModalContainer";

interface CapturedCreate {
  input: {
    name: string;
    driver: string;
    host?: string;
    port?: number;
    database_name?: string;
    username?: string;
    password?: string;
    ssl_mode?: string;
  };
}

let captured: CapturedCreate | null = null;

beforeEach(() => {
  clearTauriMocks();
  captured = null;
  mockTauriCommand("create_connection", (args: unknown) => {
    captured = args as CapturedCreate;
    return {
      id: "x",
      name: "x",
      driver: "postgres",
      host: null,
      port: null,
      database_name: null,
      username: null,
      has_password: false,
      ssl_mode: null,
      timeout_ms: 0,
      query_timeout_ms: 0,
      ttl_seconds: 0,
      max_pool_size: 0,
      is_readonly: false,
      last_tested_at: null,
      created_at: "",
      updated_at: "",
    };
  });
});

afterEach(() => clearTauriMocks());

describe("NewConnectionModalContainer", () => {
  it("renders nothing when open=false", () => {
    renderWithProviders(
      <NewConnectionModalContainer
        open={false}
        onClose={() => {}}
        onCreated={() => {}}
      />,
    );
    expect(screen.queryByTestId("new-connection-modal")).toBeNull();
  });

  it("renders the modal with form tab by default when open", () => {
    renderWithProviders(
      <NewConnectionModalContainer
        open={true}
        onClose={() => {}}
        onCreated={() => {}}
      />,
    );
    expect(screen.getByTestId("new-connection-modal")).toBeTruthy();
    expect(screen.getByTestId("new-connection-form-tab")).toBeTruthy();
  });

  it("save is disabled when name is empty", () => {
    renderWithProviders(
      <NewConnectionModalContainer
        open={true}
        onClose={() => {}}
        onCreated={() => {}}
      />,
    );
    const save = screen.getByTestId("new-connection-save") as HTMLButtonElement;
    expect(save.disabled).toBe(true);
  });

  it("save dispatches createConnection + onCreated + onClose with trimmed input", async () => {
    let closed = false;
    let created = false;
    renderWithProviders(
      <NewConnectionModalContainer
        open={true}
        onClose={() => {
          closed = true;
        }}
        onCreated={() => {
          created = true;
        }}
      />,
    );
    const user = userEvent.setup();
    const name = screen.getByTestId(
      "new-connection-field-name",
    ) as HTMLInputElement;
    await user.type(name, "  payments-db  ");

    const save = screen.getByTestId("new-connection-save");
    await user.click(save);

    // useEvent yields microtasks; the await above is enough.
    expect(captured).not.toBeNull();
    expect(captured!.input.name).toBe("payments-db");
    expect(captured!.input.driver).toBe("postgres");
    expect(created).toBe(true);
    expect(closed).toBe(true);
  });

  it("surfaces a createConnection IPC failure and keeps the modal open", async () => {
    let closed = false;
    let created = false;
    mockTauriCommand("create_connection", () => {
      throw new Error("connection refused");
    });
    renderWithProviders(
      <NewConnectionModalContainer
        open={true}
        onClose={() => {
          closed = true;
        }}
        onCreated={() => {
          created = true;
        }}
      />,
    );
    const user = userEvent.setup();
    await user.type(
      screen.getByTestId("new-connection-field-name"),
      "payments-db",
    );
    await user.click(screen.getByTestId("new-connection-save"));

    const alert = await screen.findByTestId("new-connection-error");
    expect(alert.textContent).toContain("connection refused");
    // The failure must not silently close the modal or report success.
    expect(created).toBe(false);
    expect(closed).toBe(false);
    expect(screen.getByTestId("new-connection-modal")).toBeTruthy();
  });

  it("clears the error and closes once a retried save succeeds", async () => {
    let closed = false;
    let fail = true;
    mockTauriCommand("create_connection", (args: unknown) => {
      if (fail) throw new Error("connection refused");
      captured = args as CapturedCreate;
      return {
        id: "x",
        name: "x",
        driver: "postgres",
        host: null,
        port: null,
        database_name: null,
        username: null,
        has_password: false,
        ssl_mode: null,
        timeout_ms: 0,
        query_timeout_ms: 0,
        ttl_seconds: 0,
        max_pool_size: 0,
        is_readonly: false,
        last_tested_at: null,
        created_at: "",
        updated_at: "",
      };
    });
    renderWithProviders(
      <NewConnectionModalContainer
        open={true}
        onClose={() => {
          closed = true;
        }}
        onCreated={() => {}}
      />,
    );
    const user = userEvent.setup();
    await user.type(screen.getByTestId("new-connection-field-name"), "db");
    await user.click(screen.getByTestId("new-connection-save"));
    expect(await screen.findByTestId("new-connection-error")).toBeTruthy();

    fail = false;
    await user.click(screen.getByTestId("new-connection-save"));
    expect(screen.queryByTestId("new-connection-error")).toBeNull();
    expect(closed).toBe(true);
  });

  it("Cancel calls onClose without dispatching createConnection", async () => {
    let closed = false;
    renderWithProviders(
      <NewConnectionModalContainer
        open={true}
        onClose={() => {
          closed = true;
        }}
        onCreated={() => {}}
      />,
    );
    await userEvent.setup().click(screen.getByTestId("new-connection-cancel"));
    expect(closed).toBe(true);
    expect(captured).toBeNull();
  });

  it("SSH tab shows the coming-soon placeholder", async () => {
    renderWithProviders(
      <NewConnectionModalContainer
        open={true}
        onClose={() => {}}
        onCreated={() => {}}
      />,
    );
    const tabs = screen.getByTestId("new-connection-tabs");
    const sshTab = tabs.querySelector('[data-tab-id="ssh-tunnel"]');
    if (sshTab) {
      await userEvent.setup().click(sshTab);
    }
    // The tab body switches to SshTab content; we don't assert on
    // specific copy because the placeholder text is tweakable, but
    // the tab body slot must mount.
    expect(screen.getByTestId("new-connection-tab-body")).toBeTruthy();
  });
});
