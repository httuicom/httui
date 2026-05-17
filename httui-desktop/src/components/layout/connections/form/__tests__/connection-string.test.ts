import { describe, it, expect } from "vitest";

import { buildConnectionPreview } from "@/components/layout/connections/form/connection-string";

describe("buildConnectionPreview", () => {
  it("returns the file path verbatim for sqlite", () => {
    expect(buildConnectionPreview("sqlite", "", "", "/tmp/notes.db", "")).toBe(
      "/tmp/notes.db",
    );
  });

  it("returns a placeholder when sqlite path is empty", () => {
    expect(buildConnectionPreview("sqlite", "", "", "", "")).toBe(
      "path/to/database.db",
    );
  });

  it("builds a postgres URI from fields", () => {
    expect(
      buildConnectionPreview("postgres", "db.local", "5432", "app", "alice"),
    ).toBe("postgres://alice@db.local:5432/app");
  });

  it("falls back to driver default port when port is empty", () => {
    expect(buildConnectionPreview("postgres", "x", "", "y", "z")).toBe(
      "postgres://z@x:5432/y",
    );
    expect(buildConnectionPreview("mysql", "x", "", "y", "z")).toBe(
      "mysql://z@x:3306/y",
    );
  });

  it("fills 'localhost' / 'user' / 'database' placeholders when fields are blank", () => {
    expect(buildConnectionPreview("mysql", "", "", "", "")).toBe(
      "mysql://user@localhost:3306/database",
    );
  });
});
