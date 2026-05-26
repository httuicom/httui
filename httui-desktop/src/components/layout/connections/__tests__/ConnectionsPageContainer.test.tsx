import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { act } from "@testing-library/react";
import { renderWithProviders, screen, waitFor } from "@/test/render";
import userEvent from "@testing-library/user-event";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";
import { emitTauriEvent, clearTauriListeners } from "@/test/mocks/tauri-event";

vi.mock("@/lib/theme/apply", () => ({ applyTheme: vi.fn() }));

import { ConnectionsPageContainer } from "@/components/layout/connections/ConnectionsPageContainer";
import { useWorkspaceStore } from "@/stores/workspace";
import { useSchemaCacheStore } from "@/stores/schemaCache";
import { useConnectionsStore } from "@/stores/connections";
import type { Connection } from "@/lib/tauri/connections";

const sampleList: Connection[] = [
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
  useSchemaCacheStore.setState({ byConnection: {} });
  clearTauriMocks();
  clearTauriListeners();
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
  clearTauriListeners();
  useSchemaCacheStore.setState({ byConnection: {} });
  useWorkspaceStore.setState({ vaultPath: null });
  useConnectionsStore.setState({ connections: [], loaded: false });
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

  it("opens the New Connection modal when create-new is clicked", async () => {
    renderWithProviders(<ConnectionsPageContainer />);
    await waitFor(() => {
      expect(screen.getByTestId("connections-page")).toBeTruthy();
    });
    await userEvent.setup().click(screen.getByTestId("connections-create-new"));
    expect(screen.getByTestId("new-connection-modal")).toBeTruthy();
  });

  it("does not render the legacy Test all button", async () => {
    renderWithProviders(<ConnectionsPageContainer />);
    await waitFor(() => {
      expect(screen.getByTestId("connections-page")).toBeTruthy();
    });
    expect(screen.queryByTestId("connections-test-all")).toBeNull();
  });

  it("reacts to config-changed events for connections by reloading", async () => {
    let calls = 0;
    mockTauriCommand("list_connections", () => {
      calls += 1;
      return sampleList;
    });
    renderWithProviders(<ConnectionsPageContainer />);
    await waitFor(() => {
      expect(calls).toBe(1);
    });
    emitTauriEvent("config-changed", { category: "connections" });
    await waitFor(() => {
      expect(calls).toBe(2);
    });
  });

  it("ignores config-changed events for other categories", async () => {
    let calls = 0;
    mockTauriCommand("list_connections", () => {
      calls += 1;
      return sampleList;
    });
    renderWithProviders(<ConnectionsPageContainer />);
    await waitFor(() => {
      expect(calls).toBe(1);
    });
    emitTauriEvent("config-changed", { category: "environments" });
    // Give the (no-op) handler a tick; reload must NOT fire.
    await new Promise((r) => setTimeout(r, 10));
    expect(calls).toBe(1);
  });

  it("selecting a row pre-fetches schema + runbook usages", async () => {
    const introspect = vi.fn(() => [
      {
        schema_name: "public",
        table_name: "orders",
        column_name: "id",
        data_type: "int",
      },
    ]);
    mockTauriCommand("introspect_schema", introspect);
    let usesArgs: unknown = null;
    mockTauriCommand("find_connection_uses_cmd", (args) => {
      usesArgs = args;
      return [{ file: "notes/a.md", line: 12 }];
    });
    renderWithProviders(<ConnectionsPageContainer />);
    const user = userEvent.setup();
    await user.click(await screen.findByTestId("connection-row-payments-db"));
    await waitFor(() => {
      expect(introspect).toHaveBeenCalledWith({
        connectionId: "payments-db",
      });
    });
    expect(usesArgs).toEqual({
      vaultPath: "/tmp/vault",
      connectionName: "payments-db",
    });
    // The schema cache feeding the schemaProp memo is now populated.
    await waitFor(() => {
      expect(
        useSchemaCacheStore.getState().byConnection["payments-db"]?.schema,
      ).toBeTruthy();
    });
    // The mapped usage row renders in the detail panel.
    expect(await screen.findByTestId("used-in-row-0")).toBeInTheDocument();
  });

  it("clicking a runbook-usage row forwards the file path to onNavigateFile", async () => {
    mockTauriCommand("find_connection_uses_cmd", () => [
      { file: "notes/use.md", line: 7 },
    ]);
    const onNavigateFile = vi.fn();
    renderWithProviders(
      <ConnectionsPageContainer onNavigateFile={onNavigateFile} />,
    );
    const user = userEvent.setup();
    await user.click(await screen.findByTestId("connection-row-payments-db"));
    const usageRow = await screen.findByTestId("used-in-row-0");
    await user.click(usageRow);
    expect(onNavigateFile).toHaveBeenCalledWith("notes/use.md");
  });

  it("detail-panel Edit delegates to the edit modal (onRequestEditCredentials)", async () => {
    // The container always passes onRequestEditCredentials, so the
    // credentials "Edit" button opens the modal in edit mode instead
    // of inline editing — exercising the `editing` lookup + modalOpen.
    renderWithProviders(<ConnectionsPageContainer />);
    const user = userEvent.setup();
    await user.click(await screen.findByTestId("connection-row-payments-db"));
    await user.click(await screen.findByTestId("credentials-edit"));
    const modal = await screen.findByTestId("new-connection-modal");
    expect(modal).toBeTruthy();
    // Edit mode reflects the selected connection's name.
    expect(modal.textContent).toContain("payments-db");
    await user.keyboard("{Escape}");
    await waitFor(() => {
      expect(screen.queryByTestId("new-connection-modal")).toBeNull();
    });
  });

  it("rotating the password calls update_connection with the new password", async () => {
    let updateArgs: { id?: string; input?: { password?: string } } | undefined;
    mockTauriCommand("update_connection", (args) => {
      updateArgs = args as { id?: string; input?: { password?: string } };
      return sampleList[0];
    });
    renderWithProviders(<ConnectionsPageContainer />);
    const user = userEvent.setup();
    await user.click(await screen.findByTestId("connection-row-payments-db"));
    await user.click(await screen.findByTestId("credentials-rotate"));
    await user.type(
      await screen.findByTestId("credentials-rotate-input"),
      "s3cret",
    );
    await user.click(await screen.findByTestId("credentials-rotate-save"));
    await waitFor(() => {
      expect(updateArgs).toMatchObject({
        id: "payments-db",
        input: { password: "s3cret" },
      });
    });
  });

  it("footer Test invokes test_connection for the selected connection", async () => {
    let tested: unknown = null;
    mockTauriCommand("test_connection", (args) => {
      tested = args;
      return undefined;
    });
    renderWithProviders(<ConnectionsPageContainer />);
    const user = userEvent.setup();
    await user.click(await screen.findByTestId("connection-row-payments-db"));
    await user.click(await screen.findByTestId("footer-test"));
    await waitFor(() => {
      expect(tested).toEqual({ id: "payments-db" });
    });
  });

  it("footer Duplicate creates a -copy connection then reloads", async () => {
    let createArgs: { input?: { name?: string; driver?: string } } | undefined;
    let listCalls = 0;
    mockTauriCommand("list_connections", () => {
      listCalls += 1;
      return sampleList;
    });
    mockTauriCommand("create_connection", (args) => {
      createArgs = args as { input?: { name?: string; driver?: string } };
      return sampleList[0];
    });
    renderWithProviders(<ConnectionsPageContainer />);
    const user = userEvent.setup();
    await user.click(await screen.findByTestId("connection-row-payments-db"));
    await user.click(await screen.findByTestId("footer-duplicate"));
    await waitFor(() => {
      expect(createArgs).toMatchObject({
        input: { name: "payments-db-copy", driver: "postgres" },
      });
    });
    await waitFor(() => {
      expect(listCalls).toBeGreaterThanOrEqual(2);
    });
  });

  it("footer Delete (two-step) deletes the connection then reloads", async () => {
    let deleted: unknown = null;
    let listCalls = 0;
    mockTauriCommand("list_connections", () => {
      listCalls += 1;
      return sampleList;
    });
    mockTauriCommand("delete_connection", (args) => {
      deleted = args;
      return undefined;
    });
    renderWithProviders(<ConnectionsPageContainer />);
    const user = userEvent.setup();
    await user.click(await screen.findByTestId("connection-row-payments-db"));
    // First click arms confirm; second click commits.
    await user.click(await screen.findByTestId("footer-delete"));
    await user.click(await screen.findByTestId("footer-delete"));
    await waitFor(() => {
      expect(deleted).toEqual({ id: "payments-db" });
    });
    await waitFor(() => {
      expect(listCalls).toBeGreaterThanOrEqual(2);
    });
  });

  it("the ⋮ row Test action calls test_connection directly", async () => {
    let tested: unknown = null;
    mockTauriCommand("test_connection", (args) => {
      tested = args;
      return undefined;
    });
    renderWithProviders(<ConnectionsPageContainer />);
    const user = userEvent.setup();
    await user.click(
      await screen.findByTestId("connection-row-payments-db-more"),
    );
    await user.click(await screen.findByText("Test"));
    await waitFor(() => {
      expect(tested).toEqual({ id: "payments-db" });
    });
  });

  it("the ⋮ row Edit action opens the modal in edit mode and closes it", async () => {
    renderWithProviders(<ConnectionsPageContainer />);
    const user = userEvent.setup();
    await user.click(
      await screen.findByTestId("connection-row-payments-db-more"),
    );
    await user.click(await screen.findByText("Edit"));
    expect(await screen.findByTestId("new-connection-modal")).toBeTruthy();
    // Close it — exercises the onClose (setNewOpen(false)+setEditingId(null)).
    await user.keyboard("{Escape}");
    await waitFor(() => {
      expect(screen.queryByTestId("new-connection-modal")).toBeNull();
    });
  });

  it("creating a connection from the modal triggers a reload then closes", async () => {
    let listCalls = 0;
    let created: { input?: { name?: string } } | undefined;
    mockTauriCommand("list_connections", () => {
      listCalls += 1;
      return sampleList;
    });
    mockTauriCommand("create_connection", (args) => {
      created = args as { input?: { name?: string } };
      return sampleList[0];
    });
    renderWithProviders(<ConnectionsPageContainer />);
    const user = userEvent.setup();
    await waitFor(() => {
      expect(screen.getByTestId("connections-page")).toBeTruthy();
    });
    await user.click(screen.getByTestId("connections-create-new"));
    expect(await screen.findByTestId("new-connection-modal")).toBeTruthy();
    // Save is gated on a non-empty name; fill it then submit.
    await user.type(
      await screen.findByTestId("new-connection-field-name"),
      "fresh-db",
    );
    const before = listCalls;
    await user.click(await screen.findByTestId("new-connection-save"));
    await waitFor(() => {
      expect(created).toMatchObject({ input: { name: "fresh-db" } });
    });
    // onCreated → reload (list_connections re-called).
    await waitFor(() => {
      expect(listCalls).toBeGreaterThan(before);
    });
    // onClose → modal unmounts.
    await waitFor(() => {
      expect(screen.queryByTestId("new-connection-modal")).toBeNull();
    });
  });

  it("refreshing the schema preview delegates to the schema cache store", async () => {
    const refreshSpy = vi.spyOn(useSchemaCacheStore.getState(), "refresh");
    renderWithProviders(<ConnectionsPageContainer />);
    const user = userEvent.setup();
    await user.click(await screen.findByTestId("connection-row-payments-db"));
    const refreshBtn = await screen.findByTestId("schema-refresh");
    await user.click(refreshBtn);
    await waitFor(() => {
      expect(refreshSpy).toHaveBeenCalledWith("payments-db");
    });
    refreshSpy.mockRestore();
  });

  it("deleting the selected connection clears the selection", async () => {
    renderWithProviders(<ConnectionsPageContainer />);
    const user = userEvent.setup();
    await user.click(await screen.findByTestId("connection-row-payments-db"));
    expect(
      await screen.findByTestId("connections-detail-loaded"),
    ).toBeInTheDocument();
    await user.click(await screen.findByTestId("footer-delete"));
    await user.click(await screen.findByTestId("footer-delete"));
    // selectedId === id → setSelectedId(null) → detail panel empties.
    await waitFor(() => {
      expect(screen.queryByTestId("connections-detail-loaded")).toBeNull();
    });
  });

  it("does not re-grep usages when the connections array churns but the selection is unchanged (B4)", async () => {
    let usesCalls = 0;
    mockTauriCommand("find_connection_uses_cmd", () => {
      usesCalls += 1;
      return [];
    });
    renderWithProviders(<ConnectionsPageContainer />);
    const user = userEvent.setup();
    await user.click(await screen.findByTestId("connection-row-payments-db"));
    // Selection fired the prefetch exactly once.
    await waitFor(() => expect(usesCalls).toBe(1));

    // Simulate an unrelated store refresh (test-ping / CRUD /
    // config-changed) emitting a fresh array with identical data —
    // the selected connection's name is unchanged.
    act(() => {
      useConnectionsStore.setState({
        connections: [{ ...sampleList[0] }],
        loaded: true,
      });
    });
    // Flush any effects the re-render might have scheduled.
    await act(async () => {
      await Promise.resolve();
    });

    // The grep must NOT have re-run — pre-B4 it keyed on the whole
    // `connections` array and re-fired here.
    expect(usesCalls).toBe(1);
    expect(
      screen.queryByTestId("connections-detail-loaded"),
    ).toBeInTheDocument();
  });
});
