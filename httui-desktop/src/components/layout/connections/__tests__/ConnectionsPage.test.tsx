import { describe, it, expect, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";

import { ConnectionsPage } from "@/components/layout/connections/ConnectionsPage";
import type { Connection } from "@/lib/tauri/connections";

function conn(
  id: string,
  name: string,
  driver: Connection["driver"] = "postgres",
): Connection {
  return {
    id,
    name,
    driver,
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
}

describe("ConnectionsPage", () => {
  it("composes the kind sidebar, list panel, and detail panel", () => {
    renderWithProviders(<ConnectionsPage />);
    expect(screen.getByTestId("connections-page")).toBeInTheDocument();
    expect(screen.getByTestId("connections-kind-sidebar")).toBeInTheDocument();
    expect(screen.getByTestId("connections-list-panel")).toBeInTheDocument();
    expect(screen.getByTestId("connections-detail-panel")).toBeInTheDocument();
  });

  it("renders zero status when no props are passed", () => {
    renderWithProviders(<ConnectionsPage />);
    const status = screen.getByTestId("connections-list-status");
    expect(status.textContent).toContain("0");
    expect(status.textContent).toContain("0 ok");
  });

  it("renders the keychain hint card", () => {
    renderWithProviders(<ConnectionsPage />);
    expect(screen.getByTestId("connections-keychain-hint")).toBeInTheDocument();
  });

  it("clicking a kind row toggles selection (no crash)", async () => {
    renderWithProviders(<ConnectionsPage countsByKind={{ postgres: 2 }} />);
    const row = screen.getByTestId("kind-row-postgres");
    expect(row.getAttribute("data-selected")).toBe("false");
    await userEvent.setup().click(row);
    expect(row.getAttribute("data-selected")).toBe("true");
  });

  it("forwards onCreateNew when supplied", async () => {
    const onCreateNew = vi.fn();
    renderWithProviders(<ConnectionsPage onCreateNew={onCreateNew} />);
    await userEvent.setup().click(screen.getByTestId("connections-create-new"));
    expect(onCreateNew).toHaveBeenCalledTimes(1);
  });

  it("typing in the search box updates the input", async () => {
    renderWithProviders(<ConnectionsPage />);
    const search = screen.getByTestId("connections-search") as HTMLInputElement;
    await userEvent.setup().type(search, "prod");
    expect(search.value).toBe("prod");
  });

  it("renders the per-environment section when envs are supplied", () => {
    renderWithProviders(
      <ConnectionsPage envs={[{ name: "staging", status: "ok", count: 5 }]} />,
    );
    expect(screen.getByTestId("env-row-staging")).toBeInTheDocument();
  });

  it("derives sidebar counts from a real connections list", () => {
    renderWithProviders(
      <ConnectionsPage
        connections={[
          conn("a", "x", "postgres"),
          conn("b", "y", "postgres"),
          conn("c", "z", "mysql"),
        ]}
      />,
    );
    expect(screen.getByTestId("kind-row-postgres").textContent).toContain("2");
    expect(screen.getByTestId("kind-row-mysql").textContent).toContain("1");
  });

  it("renders one list row per connection", () => {
    renderWithProviders(
      <ConnectionsPage
        connections={[
          conn("a", "alpha", "postgres"),
          conn("b", "beta", "mysql"),
        ]}
      />,
    );
    expect(screen.getByTestId("connection-row-a")).toBeInTheDocument();
    expect(screen.getByTestId("connection-row-b")).toBeInTheDocument();
    // Status counts derived from rows: 2 total, 0 ok (no enrichment).
    const status = screen.getByTestId("connections-list-status");
    expect(status.textContent).toContain("2");
  });

  it("filters list rows by sidebar kind selection", async () => {
    renderWithProviders(
      <ConnectionsPage
        connections={[
          conn("a", "alpha", "postgres"),
          conn("b", "beta", "mysql"),
        ]}
      />,
    );
    expect(screen.getByTestId("connection-row-a")).toBeInTheDocument();
    expect(screen.getByTestId("connection-row-b")).toBeInTheDocument();

    await userEvent.setup().click(screen.getByTestId("kind-row-postgres"));

    expect(screen.getByTestId("connection-row-a")).toBeInTheDocument();
    expect(screen.queryByTestId("connection-row-b")).toBeNull();
  });

  it("filters list rows by search input", async () => {
    renderWithProviders(
      <ConnectionsPage
        connections={[
          conn("a", "alpha", "postgres"),
          conn("b", "beta", "mysql"),
        ]}
      />,
    );
    await userEvent
      .setup()
      .type(screen.getByTestId("connections-search"), "alp");
    expect(screen.getByTestId("connection-row-a")).toBeInTheDocument();
    expect(screen.queryByTestId("connection-row-b")).toBeNull();
  });

  it("clicking a row updates the detail-panel placeholder name", async () => {
    renderWithProviders(
      <ConnectionsPage connections={[conn("a", "alpha-conn", "postgres")]} />,
    );
    // Detail panel starts on empty state
    expect(screen.getByTestId("connections-detail-empty")).toBeInTheDocument();
    await userEvent.setup().click(screen.getByTestId("connection-row-a"));
    // Slice 2 with a real Connection in the list,
    // selection routes into the loaded credentials panel.
    expect(
      screen.getByTestId("connections-detail-loaded").textContent,
    ).toContain("alpha-conn");
  });

  it("derives env summary from enrichment array", () => {
    renderWithProviders(
      <ConnectionsPage
        connections={[conn("a", "x", "postgres")]}
        enrichment={[{ id: "a", env: "staging", latencyMs: 50, uses: 3 }]}
      />,
    );
    expect(screen.getByTestId("env-row-staging")).toBeInTheDocument();
  });

  it("forwards onSaveCredentials with the selected connection id", async () => {
    const onSaveCredentials = vi.fn().mockResolvedValue(undefined);
    renderWithProviders(
      <ConnectionsPage
        connections={[conn("c1", "alpha", "postgres")]}
        onSaveCredentials={onSaveCredentials}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("connection-row-c1"));
    await user.click(screen.getByTestId("credentials-edit"));
    await user.click(screen.getByTestId("credentials-save"));
    expect(onSaveCredentials).toHaveBeenCalledWith("c1", expect.any(Object));
  });

  it("forwards onRotatePassword with the selected connection id", async () => {
    const onRotatePassword = vi.fn().mockResolvedValue(undefined);
    renderWithProviders(
      <ConnectionsPage
        connections={[conn("c1", "alpha", "postgres")]}
        onRotatePassword={onRotatePassword}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("connection-row-c1"));
    await user.click(screen.getByTestId("credentials-rotate"));
    await user.type(screen.getByTestId("credentials-rotate-input"), "new-pw");
    await user.click(screen.getByTestId("credentials-rotate-save"));
    expect(onRotatePassword).toHaveBeenCalledWith("c1", "new-pw");
  });

  it("status header reflects enrichment-derived intents", () => {
    renderWithProviders(
      <ConnectionsPage
        connections={[
          conn("a", "x", "postgres"),
          conn("b", "y", "mysql"),
          conn("c", "z", "postgres"),
        ]}
        enrichment={[
          { id: "a", env: null, latencyMs: 10, uses: 0 }, // ok
          { id: "b", env: null, latencyMs: 300, uses: 0 }, // slow
          { id: "c", env: null, latencyMs: -1, uses: 0 }, // down
        ]}
      />,
    );
    const status = screen.getByTestId("connections-list-status");
    expect(status.textContent).toContain("1 ok");
    expect(status.textContent).toContain("1 slow");
    expect(status.textContent).toContain("1 down");
  });
});
