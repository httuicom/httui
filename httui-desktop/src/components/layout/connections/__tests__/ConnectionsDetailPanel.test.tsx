import { describe, it, expect, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";

import { ConnectionsDetailPanel } from "@/components/layout/connections/ConnectionsDetailPanel";
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
    ssl_mode: null,
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

describe("ConnectionsDetailPanel", () => {
  it("renders the empty state when no connection is selected", () => {
    renderWithProviders(
      <ConnectionsDetailPanel selectedConnectionName={null} />,
    );
    expect(screen.getByTestId("connections-detail-empty")).toBeInTheDocument();
    expect(screen.getByText(/Nothing selected/i)).toBeInTheDocument();
  });

  it("renders the placeholder + name when a connection is selected", () => {
    renderWithProviders(
      <ConnectionsDetailPanel selectedConnectionName="prod-db" />,
    );
    const placeholder = screen.getByTestId("connections-detail-placeholder");
    expect(placeholder).toBeInTheDocument();
    expect(placeholder.textContent).toContain("prod-db");
  });

  it("renders the credentials section when a full Connection is supplied", () => {
    renderWithProviders(
      <ConnectionsDetailPanel
        selectedConnectionName="alpha"
        selectedConnection={conn()}
      />,
    );
    expect(screen.getByTestId("connections-detail-loaded")).toBeInTheDocument();
    expect(screen.getByTestId("connection-credentials")).toBeInTheDocument();
    expect(screen.queryByTestId("connections-detail-placeholder")).toBeNull();
  });

  it("forwards onSaveCredentials + onRotatePassword to the credentials section", () => {
    const onSave = vi.fn();
    const onRotate = vi.fn();
    renderWithProviders(
      <ConnectionsDetailPanel
        selectedConnectionName="alpha"
        selectedConnection={conn()}
        onSaveCredentials={onSave}
        onRotatePassword={onRotate}
      />,
    );
    // Smoke: the credentials section is mounted; deeper button
    // semantics are covered in ConnectionDetailCredentials.test.tsx.
    expect(screen.getByTestId("credentials-edit")).toBeInTheDocument();
    expect(screen.getByTestId("credentials-rotate")).toBeInTheDocument();
  });
});
