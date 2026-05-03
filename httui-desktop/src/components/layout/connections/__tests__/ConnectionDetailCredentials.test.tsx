import { describe, it, expect, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";

import { ConnectionDetailCredentials } from "@/components/layout/connections/ConnectionDetailCredentials";
import type { Connection } from "@/lib/tauri/connections";

function conn(overrides: Partial<Connection> = {}): Connection {
  return {
    id: "c1",
    name: "alpha",
    driver: "postgres",
    host: "db.local",
    port: 5432,
    database_name: "app",
    username: "alice",
    has_password: true,
    ssl_mode: "disable",
    timeout_ms: 0,
    query_timeout_ms: 0,
    ttl_seconds: 0,
    max_pool_size: 0,
    is_readonly: false,
    last_tested_at: null,
    created_at: "",
    updated_at: "",
    ...overrides,
  };
}

describe("ConnectionDetailCredentials — read mode", () => {
  it("shows host / port / user / database in the summary", () => {
    renderWithProviders(
      <ConnectionDetailCredentials
        connection={conn()}
        onSave={vi.fn()}
        onRotatePassword={vi.fn()}
      />,
    );
    const ro = screen.getByTestId("credentials-readonly");
    expect(ro.textContent).toContain("db.local");
    expect(ro.textContent).toContain("5432");
    expect(ro.textContent).toContain("alice");
    expect(ro.textContent).toContain("app");
  });

  it("masks the password as 8 bullets", () => {
    renderWithProviders(
      <ConnectionDetailCredentials
        connection={conn()}
        onSave={vi.fn()}
        onRotatePassword={vi.fn()}
      />,
    );
    const row = screen.getByTestId("credentials-row-password");
    expect(row.textContent).toContain("••••••••");
  });

  it("shows '—' for null fields", () => {
    renderWithProviders(
      <ConnectionDetailCredentials
        connection={conn({ host: null, port: null, username: null, database_name: null })}
        onSave={vi.fn()}
        onRotatePassword={vi.fn()}
      />,
    );
    expect(
      screen.getByTestId("credentials-row-host").textContent,
    ).toContain("—");
    expect(
      screen.getByTestId("credentials-row-port").textContent,
    ).toContain("—");
    expect(
      screen.getByTestId("credentials-row-user").textContent,
    ).toContain("—");
    expect(
      screen.getByTestId("credentials-row-database").textContent,
    ).toContain("—");
  });
});

describe("ConnectionDetailCredentials — edit mode", () => {
  it("Edit toggles the field inputs and Save/Cancel buttons", async () => {
    renderWithProviders(
      <ConnectionDetailCredentials
        connection={conn()}
        onSave={vi.fn()}
        onRotatePassword={vi.fn()}
      />,
    );
    expect(
      screen.queryByTestId("credentials-editing"),
    ).toBeNull();
    await userEvent.setup().click(screen.getByTestId("credentials-edit"));
    expect(
      screen.getByTestId("credentials-editing"),
    ).toBeInTheDocument();
    expect(screen.getByTestId("credentials-save")).toBeInTheDocument();
    expect(screen.getByTestId("credentials-cancel")).toBeInTheDocument();
  });

  it("Cancel reverts edits without calling onSave", async () => {
    const onSave = vi.fn();
    renderWithProviders(
      <ConnectionDetailCredentials
        connection={conn()}
        onSave={onSave}
        onRotatePassword={vi.fn()}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("credentials-edit"));
    const host = screen.getByTestId("credentials-host") as HTMLInputElement;
    await user.clear(host);
    await user.type(host, "other.local");
    expect(host.value).toBe("other.local");
    await user.click(screen.getByTestId("credentials-cancel"));
    expect(onSave).not.toHaveBeenCalled();
    expect(
      screen.getByTestId("credentials-readonly").textContent,
    ).toContain("db.local");
  });

  it("Save dispatches onSave with the edited fields and exits edit mode", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    renderWithProviders(
      <ConnectionDetailCredentials
        connection={conn()}
        onSave={onSave}
        onRotatePassword={vi.fn()}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("credentials-edit"));
    const host = screen.getByTestId("credentials-host") as HTMLInputElement;
    await user.clear(host);
    await user.type(host, "newhost");
    await user.click(screen.getByTestId("credentials-save"));
    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({ host: "newhost" }),
    );
    // Wait microtask for the promise to settle and the button toggle.
    await new Promise((r) => setTimeout(r, 10));
    expect(
      screen.queryByTestId("credentials-editing"),
    ).toBeNull();
  });

  it("surfaces the save error and stays in edit mode on failure", async () => {
    const onSave = vi.fn().mockRejectedValue(new Error("conflict"));
    renderWithProviders(
      <ConnectionDetailCredentials
        connection={conn()}
        onSave={onSave}
        onRotatePassword={vi.fn()}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("credentials-edit"));
    await user.click(screen.getByTestId("credentials-save"));
    await new Promise((r) => setTimeout(r, 10));
    expect(
      screen.getByTestId("credentials-save-error").textContent,
    ).toContain("conflict");
    expect(
      screen.getByTestId("credentials-editing"),
    ).toBeInTheDocument();
  });

  it("blank port → input.port is undefined in the save payload", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    renderWithProviders(
      <ConnectionDetailCredentials
        connection={conn()}
        onSave={onSave}
        onRotatePassword={vi.fn()}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("credentials-edit"));
    const port = screen.getByTestId("credentials-port") as HTMLInputElement;
    await user.clear(port);
    await user.click(screen.getByTestId("credentials-save"));
    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({ port: undefined }),
    );
  });
});

describe("ConnectionDetailCredentials — rotate password", () => {
  it("Rotate button reveals the password input + Save/Cancel", async () => {
    renderWithProviders(
      <ConnectionDetailCredentials
        connection={conn()}
        onSave={vi.fn()}
        onRotatePassword={vi.fn()}
      />,
    );
    expect(
      screen.queryByTestId("credentials-rotate-input"),
    ).toBeNull();
    await userEvent.setup().click(screen.getByTestId("credentials-rotate"));
    expect(
      screen.getByTestId("credentials-rotate-input"),
    ).toBeInTheDocument();
  });

  it("rejects empty new password", async () => {
    const onRotatePassword = vi.fn();
    renderWithProviders(
      <ConnectionDetailCredentials
        connection={conn()}
        onSave={vi.fn()}
        onRotatePassword={onRotatePassword}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("credentials-rotate"));
    await user.click(screen.getByTestId("credentials-rotate-save"));
    expect(onRotatePassword).not.toHaveBeenCalled();
    expect(
      screen.getByTestId("credentials-rotate-error").textContent,
    ).toContain("empty");
  });

  it("dispatches onRotatePassword with the new password and closes the section", async () => {
    const onRotatePassword = vi.fn().mockResolvedValue(undefined);
    renderWithProviders(
      <ConnectionDetailCredentials
        connection={conn()}
        onSave={vi.fn()}
        onRotatePassword={onRotatePassword}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("credentials-rotate"));
    await user.type(
      screen.getByTestId("credentials-rotate-input"),
      "new-secret",
    );
    await user.click(screen.getByTestId("credentials-rotate-save"));
    expect(onRotatePassword).toHaveBeenCalledWith("new-secret");
    await new Promise((r) => setTimeout(r, 10));
    expect(
      screen.queryByTestId("credentials-rotate-input"),
    ).toBeNull();
  });

  it("Cancel closes the rotate section without dispatching", async () => {
    const onRotatePassword = vi.fn();
    renderWithProviders(
      <ConnectionDetailCredentials
        connection={conn()}
        onSave={vi.fn()}
        onRotatePassword={onRotatePassword}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("credentials-rotate"));
    await user.type(
      screen.getByTestId("credentials-rotate-input"),
      "secret",
    );
    await user.click(screen.getByTestId("credentials-rotate-cancel"));
    expect(onRotatePassword).not.toHaveBeenCalled();
    expect(
      screen.queryByTestId("credentials-rotate-input"),
    ).toBeNull();
  });

  it("surfaces rotate error and stays open on failure", async () => {
    const onRotatePassword = vi
      .fn()
      .mockRejectedValue(new Error("keychain locked"));
    renderWithProviders(
      <ConnectionDetailCredentials
        connection={conn()}
        onSave={vi.fn()}
        onRotatePassword={onRotatePassword}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("credentials-rotate"));
    await user.type(
      screen.getByTestId("credentials-rotate-input"),
      "x",
    );
    await user.click(screen.getByTestId("credentials-rotate-save"));
    await new Promise((r) => setTimeout(r, 10));
    expect(
      screen.getByTestId("credentials-rotate-error").textContent,
    ).toContain("keychain locked");
    expect(
      screen.getByTestId("credentials-rotate-input"),
    ).toBeInTheDocument();
  });
});

describe("ConnectionDetailCredentials — connection switching", () => {
  it("resets edit state and draft when the connection changes", async () => {
    const { rerender } = renderWithProviders(
      <ConnectionDetailCredentials
        connection={conn({ id: "a", name: "alpha", host: "a.local" })}
        onSave={vi.fn()}
        onRotatePassword={vi.fn()}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("credentials-edit"));
    expect(
      screen.getByTestId("credentials-editing"),
    ).toBeInTheDocument();
    rerender(
      <ConnectionDetailCredentials
        connection={conn({ id: "b", name: "beta", host: "b.local" })}
        onSave={vi.fn()}
        onRotatePassword={vi.fn()}
      />,
    );
    expect(
      screen.queryByTestId("credentials-editing"),
    ).toBeNull();
    expect(
      screen.getByTestId("credentials-readonly").textContent,
    ).toContain("b.local");
  });
});
