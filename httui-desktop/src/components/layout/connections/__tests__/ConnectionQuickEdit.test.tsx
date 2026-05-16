import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";

import { ConnectionQuickEdit } from "@/components/layout/connections/ConnectionQuickEdit";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";
import { useConnectionSessionOverrideStore } from "@/stores/connectionSessionOverride";

const conn = {
  id: "c1",
  name: "local-pg",
  driver: "postgres" as const,
  host: "localhost",
  port: 5432,
  database_name: "db",
  username: "u",
  has_password: true,
  ssl_mode: null,
  timeout_ms: 5000,
  query_timeout_ms: 30000,
  ttl_seconds: 30,
  max_pool_size: 10,
  is_readonly: false,
  last_tested_at: null,
  created_at: "2026-01-01T00:00:00Z",
  updated_at: "2026-01-01T00:00:00Z",
};

const noop = () => {};

beforeEach(() => {
  clearTauriMocks();
  useConnectionSessionOverrideStore.setState({ overrides: {} });
});
afterEach(() => clearTauriMocks());

describe("ConnectionQuickEdit", () => {
  it("Save is disabled until a password is typed", async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <ConnectionQuickEdit
        conn={conn}
        onTest={noop}
        onEdit={noop}
        onDelete={noop}
        onDuplicate={noop}
        onChanged={noop}
      />,
    );
    const save = screen.getByTestId("conn-quickedit-rotate");
    expect(save).toBeDisabled();
    await user.type(screen.getByTestId("conn-quickedit-password"), "x");
    expect(save).not.toBeDisabled();
  });

  it("rotate failure surfaces the error message", async () => {
    const user = userEvent.setup();
    mockTauriCommand("update_connection", () => {
      throw new Error("keychain locked");
    });
    renderWithProviders(
      <ConnectionQuickEdit
        conn={conn}
        onTest={noop}
        onEdit={noop}
        onDelete={noop}
        onDuplicate={noop}
        onChanged={noop}
      />,
    );
    await user.type(screen.getByTestId("conn-quickedit-password"), "pw");
    await user.click(screen.getByTestId("conn-quickedit-rotate"));
    expect(
      (await screen.findByTestId("conn-quickedit-rotate-msg")).textContent,
    ).toContain("keychain locked");
  });

  it("prefills host/port from an existing override and clears it", async () => {
    const user = userEvent.setup();
    useConnectionSessionOverrideStore
      .getState()
      .setOverride("c1", { host: "db.staging", port: 5599 });
    renderWithProviders(
      <ConnectionQuickEdit
        conn={conn}
        onTest={noop}
        onEdit={noop}
        onDelete={noop}
        onDuplicate={noop}
        onChanged={noop}
      />,
    );
    expect(
      (screen.getByTestId("conn-quickedit-host") as HTMLInputElement).value,
    ).toBe("db.staging");
    expect(
      (screen.getByTestId("conn-quickedit-port") as HTMLInputElement).value,
    ).toBe("5599");
    // TemporaryChip is interactive here — clicking it clears the override.
    await user.click(screen.getByTestId("temporary-chip"));
    expect(
      useConnectionSessionOverrideStore.getState().getOverride("c1"),
    ).toBeUndefined();
  });

  it("calls onChanged after a successful rotate", async () => {
    const user = userEvent.setup();
    const onChanged = vi.fn();
    mockTauriCommand("update_connection", () => ({ ...conn }));
    renderWithProviders(
      <ConnectionQuickEdit
        conn={conn}
        onTest={noop}
        onEdit={noop}
        onDelete={noop}
        onDuplicate={noop}
        onChanged={onChanged}
      />,
    );
    await user.type(screen.getByTestId("conn-quickedit-password"), "pw");
    await user.click(screen.getByTestId("conn-quickedit-rotate"));
    expect(await screen.findByTestId("conn-quickedit-rotate-msg")).toHaveTextContent(
      /updated/i,
    );
    expect(onChanged).toHaveBeenCalled();
  });
});
