// Legacy JSON bodies must be rewritten in place exactly once; raw SQL
// bodies must never be touched.
import { describe, expect, it } from "vitest";
import { renderHook } from "@testing-library/react";

import { useDbLegacyMigration } from "../useDbLegacyMigration";
import { makeDbBlock, makeView } from "./helpers";

describe("useDbLegacyMigration", () => {
  it("rewrites a legacy JSON body into raw SQL + info string", () => {
    const view = makeView("");
    const legacyBody = JSON.stringify({
      connectionId: "prod",
      query: "SELECT * FROM users",
      limit: 50,
      timeoutMs: 9000,
    });
    const block = makeDbBlock(view, {}, legacyBody);

    renderHook(() => useDbLegacyMigration(block, view));

    const doc = view.state.doc.toString();
    expect(doc).toContain("SELECT * FROM users");
    expect(doc).not.toContain('"connectionId"');
    expect(doc).toContain("connection=prod");
    expect(doc).toContain("limit=50");
    expect(doc).toContain("timeout=9000");
  });

  it("keeps explicit metadata over legacy values", () => {
    const view = makeView("");
    const legacyBody = JSON.stringify({
      connectionId: "legacy-conn",
      query: "SELECT 1",
    });
    const block = makeDbBlock(view, { connection: "explicit" }, legacyBody);

    renderHook(() => useDbLegacyMigration(block, view));

    const doc = view.state.doc.toString();
    expect(doc).toContain("connection=explicit");
    expect(doc).not.toContain("legacy-conn");
  });

  it("leaves raw SQL bodies untouched", () => {
    const view = makeView("");
    const block = makeDbBlock(view, {}, "SELECT 1;");
    const before = view.state.doc.toString();

    renderHook(() => useDbLegacyMigration(block, view));

    expect(view.state.doc.toString()).toBe(before);
  });

  it("does not re-run for the same body", () => {
    const view = makeView("");
    const legacyBody = JSON.stringify({ connectionId: "p", query: "SELECT 2" });
    const block = makeDbBlock(view, {}, legacyBody);

    const { rerender } = renderHook(() => useDbLegacyMigration(block, view));
    const afterFirst = view.state.doc.toString();
    rerender();

    expect(view.state.doc.toString()).toBe(afterFirst);
  });
});
