import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderWithProviders, screen, waitFor } from "@/test/render";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";

vi.mock("@/lib/theme/apply", () => ({ applyTheme: vi.fn() }));

import { ConnectionsPageContainer } from "@/components/layout/connections/ConnectionsPageContainer";
import { useWorkspaceStore } from "@/stores/workspace";

const sampleList = [
  {
    id: "payments-db",
    name: "payments-db",
    driver: "postgres",
    host: "pg.local",
    port: 5432,
    database_name: "payments",
    username: "app",
    has_password: true,
    ssl_mode: null,
    timeout_ms: 10000,
    query_timeout_ms: 30000,
    ttl_seconds: 300,
    max_pool_size: 5,
    is_readonly: false,
    last_tested_at: null,
    created_at: "",
    updated_at: "",
  },
];

beforeEach(() => {
  useWorkspaceStore.setState({ vaultPath: "/tmp/vault" });
  clearTauriMocks();
  mockTauriCommand("list_connections", () => sampleList);
  mockTauriCommand("find_connection_uses_cmd", () => []);
  mockTauriCommand("test_connection", () => undefined);
  mockTauriCommand("update_connection", () => sampleList[0]);
  mockTauriCommand("create_connection", () => sampleList[0]);
  mockTauriCommand("delete_connection", () => undefined);
  mockTauriCommand("introspect_schema", () => []);
  mockTauriCommand("get_cached_schema", () => null);
});

afterEach(() => {
  clearTauriMocks();
  useWorkspaceStore.setState({ vaultPath: null });
});

describe("ConnectionsPageContainer", () => {
  it("renders the underlying ConnectionsPage", async () => {
    renderWithProviders(<ConnectionsPageContainer />);
    await waitFor(() => {
      expect(screen.getByTestId("connections-page")).toBeTruthy();
    });
  });

  it("loads connections via listConnections and exposes them to the page", async () => {
    let calls = 0;
    mockTauriCommand("list_connections", () => {
      calls += 1;
      return sampleList;
    });
    renderWithProviders(<ConnectionsPageContainer />);
    await waitFor(() => {
      expect(calls).toBe(1);
    });
  });

  it("survives an empty connection list", async () => {
    mockTauriCommand("list_connections", () => []);
    renderWithProviders(<ConnectionsPageContainer />);
    await waitFor(() => {
      expect(screen.getByTestId("connections-page")).toBeTruthy();
    });
  });
});
