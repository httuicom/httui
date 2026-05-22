import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderWithProviders, screen, waitFor } from "@/test/render";
import userEvent from "@testing-library/user-event";

import { ConnectionsList } from "@/components/layout/connections/ConnectionsList";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";
import { useConnectionSessionOverrideStore } from "@/stores/connectionSessionOverride";

const mkConn = (id: string, name: string, driver = "postgres") => ({
  id,
  name,
  driver,
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
});

beforeEach(() => {
  clearTauriMocks();
  // Default: empty list, every test_connection succeeds.
  mockTauriCommand("list_connections", () => []);
  mockTauriCommand("test_connection", async () => undefined);
  mockTauriCommand("delete_connection", async () => undefined);
  useConnectionSessionOverrideStore.setState({ overrides: {} });
});

afterEach(() => {
  clearTauriMocks();
});

describe("ConnectionsList", () => {
  it("renders the Connections section header", async () => {
    renderWithProviders(<ConnectionsList />);
    expect(screen.getByText("Connections")).toBeInTheDocument();
  });

  it('shows "No connections" placeholder when the list is empty', async () => {
    renderWithProviders(<ConnectionsList />);
    await waitFor(() =>
      expect(screen.getByText("No connections")).toBeInTheDocument(),
    );
  });

  it("renders each connection row from list_connections", async () => {
    mockTauriCommand("list_connections", () => [
      mkConn("c1", "local-pg"),
      mkConn("c2", "prod-mysql", "mysql"),
    ]);
    renderWithProviders(<ConnectionsList />);
    await waitFor(() =>
      expect(screen.getByText("local-pg")).toBeInTheDocument(),
    );
    expect(screen.getByText("prod-mysql")).toBeInTheDocument();
  });

  it("renders a PROD badge when the name matches /prod/i", async () => {
    mockTauriCommand("list_connections", () => [
      mkConn("c1", "local-pg"),
      mkConn("c2", "PROD-mysql", "mysql"),
      mkConn("c3", "staging-rep", "postgres"),
    ]);
    renderWithProviders(<ConnectionsList />);
    await waitFor(() =>
      expect(screen.getByText("PROD-mysql")).toBeInTheDocument(),
    );
    expect(
      screen.getByTestId("sidebar-connection-c2-prod"),
    ).toBeInTheDocument();
    expect(screen.queryByTestId("sidebar-connection-c1-prod")).toBeNull();
    expect(screen.queryByTestId("sidebar-connection-c3-prod")).toBeNull();
  });

  it("auto-pings each connection on mount and shows latency + ok dot", async () => {
    mockTauriCommand("list_connections", () => [mkConn("c1", "local-pg")]);
    mockTauriCommand("test_connection", async () => undefined);
    renderWithProviders(<ConnectionsList />);

    await waitFor(() =>
      expect(screen.getByText("local-pg")).toBeInTheDocument(),
    );

    await waitFor(() =>
      expect(
        screen.getByTestId("sidebar-connection-c1-latency"),
      ).toBeInTheDocument(),
    );
    const row = screen.getByTestId("sidebar-connection-c1");
    expect(row.getAttribute("data-status")).toBe("ok");
  });

  it("shows the err dot when test_connection rejects", async () => {
    mockTauriCommand("list_connections", () => [mkConn("c1", "local-pg")]);
    mockTauriCommand("test_connection", () => {
      throw new Error("connection refused");
    });
    renderWithProviders(<ConnectionsList />);

    await waitFor(() => {
      const row = screen.getByTestId("sidebar-connection-c1");
      expect(row.getAttribute("data-status")).toBe("err");
    });
  });

  it("opens the form on +", async () => {
    const user = userEvent.setup();
    renderWithProviders(<ConnectionsList />);
    await user.click(screen.getByLabelText("New connection"));
    expect(screen.getByRole("button", { name: /Create/i })).toBeInTheDocument();
  });

  it("clicking the chip opens the quick-edit popover", async () => {
    const user = userEvent.setup();
    mockTauriCommand("list_connections", () => [mkConn("c1", "local-pg")]);
    renderWithProviders(<ConnectionsList />);
    await waitFor(() =>
      expect(screen.getByText("local-pg")).toBeInTheDocument(),
    );
    await user.click(screen.getByTestId("sidebar-connection-c1"));
    expect(await screen.findByTestId("conn-quickedit-c1")).toBeInTheDocument();
    expect(screen.getByTestId("conn-quickedit-rotate")).toBeInTheDocument();
    expect(screen.getByTestId("conn-quickedit-apply")).toBeInTheDocument();
    expect(screen.getByTestId("conn-quickedit-test")).toBeInTheDocument();
    expect(screen.getByTestId("conn-quickedit-duplicate")).toBeInTheDocument();
  });

  it("Delete in the popover calls delete_connection then refresh", async () => {
    const user = userEvent.setup();
    const deleteSpy = vi.fn();
    mockTauriCommand("list_connections", () => [mkConn("c1", "local-pg")]);
    mockTauriCommand("delete_connection", (args) => {
      deleteSpy(args);
      return undefined;
    });
    renderWithProviders(<ConnectionsList />);
    await waitFor(() =>
      expect(screen.getByText("local-pg")).toBeInTheDocument(),
    );
    await user.click(screen.getByTestId("sidebar-connection-c1"));
    await user.click(await screen.findByTestId("conn-quickedit-delete"));
    await waitFor(() => expect(deleteSpy).toHaveBeenCalled());
    expect(deleteSpy.mock.calls[0][0]).toMatchObject({ id: "c1" });
  });

  it("Edit in the popover opens the form preloaded with the connection", async () => {
    const user = userEvent.setup();
    mockTauriCommand("list_connections", () => [mkConn("c1", "local-pg")]);
    renderWithProviders(<ConnectionsList />);
    await waitFor(() =>
      expect(screen.getByText("local-pg")).toBeInTheDocument(),
    );
    await user.click(screen.getByTestId("sidebar-connection-c1"));
    await user.click(await screen.findByTestId("conn-quickedit-edit"));
    expect(screen.getByRole("button", { name: /Save/i })).toBeInTheDocument();
  });

  it("Test in the popover re-pings and updates latency", async () => {
    const user = userEvent.setup();
    let pingCount = 0;
    mockTauriCommand("list_connections", () => [mkConn("c1", "local-pg")]);
    mockTauriCommand("test_connection", async () => {
      pingCount += 1;
      return undefined;
    });
    renderWithProviders(<ConnectionsList />);
    await waitFor(() =>
      expect(screen.getByText("local-pg")).toBeInTheDocument(),
    );
    await waitFor(() => expect(pingCount).toBeGreaterThanOrEqual(1));
    const before = pingCount;
    await user.click(screen.getByTestId("sidebar-connection-c1"));
    await user.click(await screen.findByTestId("conn-quickedit-test"));
    await waitFor(() => expect(pingCount).toBeGreaterThan(before));
  });

  it("Rotate password sends update_connection with the new password", async () => {
    const user = userEvent.setup();
    const updateSpy = vi.fn();
    mockTauriCommand("list_connections", () => [mkConn("c1", "local-pg")]);
    mockTauriCommand("update_connection", (args) => {
      updateSpy(args);
      return mkConn("c1", "local-pg");
    });
    renderWithProviders(<ConnectionsList />);
    await waitFor(() =>
      expect(screen.getByText("local-pg")).toBeInTheDocument(),
    );
    await user.click(screen.getByTestId("sidebar-connection-c1"));
    await user.type(
      await screen.findByTestId("conn-quickedit-password"),
      "s3cret",
    );
    await user.click(screen.getByTestId("conn-quickedit-rotate"));
    await waitFor(() => expect(updateSpy).toHaveBeenCalled());
    expect(updateSpy.mock.calls[0][0]).toMatchObject({
      id: "c1",
      input: { password: "s3cret" },
    });
  });

  it("Applying a temporary host:port sets the session override + chip", async () => {
    const user = userEvent.setup();
    mockTauriCommand("list_connections", () => [mkConn("c1", "local-pg")]);
    renderWithProviders(<ConnectionsList />);
    await waitFor(() =>
      expect(screen.getByText("local-pg")).toBeInTheDocument(),
    );
    await user.click(screen.getByTestId("sidebar-connection-c1"));
    await user.type(
      await screen.findByTestId("conn-quickedit-host"),
      "db.staging",
    );
    await user.type(screen.getByTestId("conn-quickedit-port"), "5599");
    await user.click(screen.getByTestId("conn-quickedit-apply"));
    expect(
      useConnectionSessionOverrideStore.getState().getOverride("c1"),
    ).toEqual({ host: "db.staging", port: 5599 });
    expect(
      screen
        .getByTestId("sidebar-connection-c1")
        .getAttribute("data-temporary"),
    ).toBe("true");
  });

  it("Duplicate sends create_connection cloned from the source", async () => {
    const user = userEvent.setup();
    const createSpy = vi.fn();
    mockTauriCommand("list_connections", () => [mkConn("c1", "local-pg")]);
    mockTauriCommand("create_connection", (args) => {
      createSpy(args);
      return mkConn("c2", "local-pg copy");
    });
    renderWithProviders(<ConnectionsList />);
    await waitFor(() =>
      expect(screen.getByText("local-pg")).toBeInTheDocument(),
    );
    await user.click(screen.getByTestId("sidebar-connection-c1"));
    await user.click(await screen.findByTestId("conn-quickedit-duplicate"));
    await waitFor(() => expect(createSpy).toHaveBeenCalled());
    expect(createSpy.mock.calls[0][0]).toMatchObject({
      input: { name: "local-pg copy", driver: "postgres" },
    });
  });
});
