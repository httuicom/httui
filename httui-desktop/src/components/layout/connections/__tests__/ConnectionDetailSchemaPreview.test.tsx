import { describe, it, expect, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";

import {
  ConnectionDetailSchemaPreview,
  HOT_TABLES_LIMIT,
} from "@/components/layout/connections/ConnectionDetailSchemaPreview";
import type { ConnectionSchema } from "@/stores/schemaCache";

function schemaOf(
  ...names: { schema?: string | null; name: string; cols?: number }[]
): ConnectionSchema {
  return {
    fetchedAt: 0,
    tables: names.map((t) => ({
      schema: t.schema ?? null,
      name: t.name,
      columns: Array.from({ length: t.cols ?? 1 }, (_, i) => ({
        name: `c${i}`,
        dataType: "text",
      })),
    })),
  };
}

describe("ConnectionDetailSchemaPreview — empty / loading / error", () => {
  it("shows the empty hint when schema is null + not loading + no error", () => {
    renderWithProviders(
      <ConnectionDetailSchemaPreview
        schema={null}
        loading={false}
        error={null}
        hotTables={[]}
      />,
    );
    expect(screen.getByTestId("schema-empty")).toBeInTheDocument();
    expect(screen.queryByTestId("schema-tables")).toBeNull();
  });

  it("shows the loading hint when schema is null + loading", () => {
    renderWithProviders(
      <ConnectionDetailSchemaPreview
        schema={null}
        loading={true}
        error={null}
        hotTables={[]}
      />,
    );
    expect(screen.getByTestId("schema-loading")).toBeInTheDocument();
  });

  it("shows the error message when error is non-null", () => {
    renderWithProviders(
      <ConnectionDetailSchemaPreview
        schema={null}
        loading={false}
        error="permission denied"
        hotTables={[]}
      />,
    );
    expect(screen.getByTestId("schema-error").textContent).toContain(
      "permission denied",
    );
  });
});

describe("ConnectionDetailSchemaPreview — tables tree", () => {
  it("renders one table row per schema entry with column count", () => {
    renderWithProviders(
      <ConnectionDetailSchemaPreview
        schema={schemaOf(
          { schema: "public", name: "users", cols: 5 },
          { schema: "public", name: "orders", cols: 8 },
        )}
        loading={false}
        error={null}
        hotTables={[]}
      />,
    );
    expect(screen.getByTestId("schema-table-public.users")).toBeInTheDocument();
    expect(
      screen.getByTestId("schema-table-public.orders"),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId("schema-table-public.users").textContent,
    ).toContain("5 cols");
  });

  it("falls back to bare name when schema is null (sqlite)", () => {
    renderWithProviders(
      <ConnectionDetailSchemaPreview
        schema={schemaOf({ schema: null, name: "items", cols: 2 })}
        loading={false}
        error={null}
        hotTables={[]}
      />,
    );
    expect(screen.getByTestId("schema-table-items")).toBeInTheDocument();
  });

  it("renders the all-tables count line", () => {
    renderWithProviders(
      <ConnectionDetailSchemaPreview
        schema={schemaOf(
          { schema: "public", name: "a" },
          { schema: "public", name: "b" },
          { schema: "public", name: "c" },
        )}
        loading={false}
        error={null}
        hotTables={[]}
      />,
    );
    expect(screen.getByTestId("schema-tables-count").textContent).toContain(
      "3",
    );
  });

  it("clicking the toggle expands columns", async () => {
    renderWithProviders(
      <ConnectionDetailSchemaPreview
        schema={schemaOf({ schema: "public", name: "users", cols: 3 })}
        loading={false}
        error={null}
        hotTables={[]}
      />,
    );
    expect(screen.queryByTestId("schema-table-cols-public.users")).toBeNull();
    await userEvent
      .setup()
      .click(screen.getByTestId("schema-table-toggle-public.users"));
    expect(
      screen.getByTestId("schema-table-cols-public.users"),
    ).toBeInTheDocument();
  });

  it("clicking again collapses columns", async () => {
    renderWithProviders(
      <ConnectionDetailSchemaPreview
        schema={schemaOf({ schema: "public", name: "users", cols: 1 })}
        loading={false}
        error={null}
        hotTables={[]}
      />,
    );
    const toggle = screen.getByTestId("schema-table-toggle-public.users");
    const user = userEvent.setup();
    await user.click(toggle);
    expect(
      screen.getByTestId("schema-table-cols-public.users"),
    ).toBeInTheDocument();
    await user.click(toggle);
    expect(screen.queryByTestId("schema-table-cols-public.users")).toBeNull();
  });
});

describe("ConnectionDetailSchemaPreview — hot tables", () => {
  it("hides the section when hotTables is empty", () => {
    renderWithProviders(
      <ConnectionDetailSchemaPreview
        schema={null}
        loading={false}
        error={null}
        hotTables={[]}
      />,
    );
    expect(screen.queryByTestId("schema-hot-tables")).toBeNull();
  });

  it("renders one row per hot table with hits count", () => {
    renderWithProviders(
      <ConnectionDetailSchemaPreview
        schema={null}
        loading={false}
        error={null}
        hotTables={[
          { tableName: "users", hits: 12 },
          { tableName: "orders", hits: 7 },
        ]}
      />,
    );
    expect(screen.getByTestId("schema-hot-row-users").textContent).toContain(
      "12",
    );
    expect(screen.getByTestId("schema-hot-row-orders").textContent).toContain(
      "7",
    );
  });

  it(`caps the hot list at HOT_TABLES_LIMIT (${HOT_TABLES_LIMIT})`, () => {
    const long = Array.from({ length: HOT_TABLES_LIMIT + 3 }, (_, i) => ({
      tableName: `t${i}`,
      hits: 100 - i,
    }));
    renderWithProviders(
      <ConnectionDetailSchemaPreview
        schema={null}
        loading={false}
        error={null}
        hotTables={long}
      />,
    );
    for (let i = 0; i < HOT_TABLES_LIMIT; i++) {
      expect(screen.getByTestId(`schema-hot-row-t${i}`)).toBeInTheDocument();
    }
    expect(
      screen.queryByTestId(`schema-hot-row-t${HOT_TABLES_LIMIT}`),
    ).toBeNull();
  });
});

describe("ConnectionDetailSchemaPreview — refresh button", () => {
  it("hides the button when onRefresh is omitted", () => {
    renderWithProviders(
      <ConnectionDetailSchemaPreview
        schema={null}
        loading={false}
        error={null}
        hotTables={[]}
      />,
    );
    expect(screen.queryByTestId("schema-refresh")).toBeNull();
  });

  it("dispatches onRefresh when clicked", async () => {
    const onRefresh = vi.fn();
    renderWithProviders(
      <ConnectionDetailSchemaPreview
        schema={null}
        loading={false}
        error={null}
        hotTables={[]}
        onRefresh={onRefresh}
      />,
    );
    await userEvent.setup().click(screen.getByTestId("schema-refresh"));
    expect(onRefresh).toHaveBeenCalledTimes(1);
  });

  it("shows 'Loading…' label when loading", () => {
    renderWithProviders(
      <ConnectionDetailSchemaPreview
        schema={null}
        loading={true}
        error={null}
        hotTables={[]}
        onRefresh={() => {}}
      />,
    );
    expect(screen.getByTestId("schema-refresh").textContent).toContain(
      "Loading",
    );
  });
});
