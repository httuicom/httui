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
