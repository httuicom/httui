import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderWithProviders, screen, waitFor } from "@/test/render";
import userEvent from "@testing-library/user-event";

import { ConnectionsList } from "@/components/layout/connections/ConnectionsList";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";

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
    await waitFor(() => expect(screen.getByText("local-pg")).toBeInTheDocument());
    expect(screen.getByText("prod-mysql")).toBeInTheDocument();
  });

  it("renders a PROD badge when the name matches /prod/i", async () => {
    mockTauriCommand("list_connections", () => [
      mkConn("c1", "local-pg"),
      mkConn("c2", "PROD-mysql", "mysql"),
      mkConn("c3", "staging-rep", "postgres"),
    ]);
    renderWithProviders(<ConnectionsList />);
    await waitFor(() => expect(screen.getByText("PROD-mysql")).toBeInTheDocument());
    expect(screen.getByTestId("sidebar-connection-c2-prod")).toBeInTheDocument();
    expect(
      screen.queryByTestId("sidebar-connection-c1-prod"),
    ).toBeNull();
    expect(
      screen.queryByTestId("sidebar-connection-c3-prod"),
    ).toBeNull();
  });

  it("auto-pings each connection on mount and shows latency + ok dot", async () => {
    mockTauriCommand("list_connections", () => [mkConn("c1", "local-pg")]);
    mockTauriCommand("test_connection", async () => undefined);
    renderWithProviders(<ConnectionsList />);

    await waitFor(() =>
      expect(screen.getByText("local-pg")).toBeInTheDocument(),
    );

    // Wait until ping resolves.
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
    // ConnectionForm renders a Create button as part of its modal.
    // That's the cheapest signal that the form mounted.
    expect(
      screen.getByRole("button", { name: /Create/i }),
    ).toBeInTheDocument();
  });
});
