import { describe, it, expect, vi } from "vitest";
import { render } from "@testing-library/react";
import type { EditorView } from "@codemirror/view";

// Mock the heavy Panels + the cm-*-block registry trios so the
// portal registry can be imported in isolation. Each Panel becomes
// a sentinel-renderable component so we can assert wiring via the
// shared `BlockWidgetPortals` (which IS the real one — it's a thin
// generic + we want to exercise the wiring end-to-end).
vi.mock("@/components/blocks/db/fenced/DbFencedPanel", () => ({
  DbFencedPanel: ({ blockId }: { blockId: string }) => (
    <div data-testid={`db-panel-${blockId}`} />
  ),
}));
vi.mock("@/components/blocks/http/fenced/HttpFencedPanel", () => ({
  HttpFencedPanel: ({ blockId }: { blockId: string }) => (
    <div data-testid={`http-panel-${blockId}`} />
  ),
}));

function makeFakeRegistry<E extends { block: unknown }>() {
  const entries = new Map<string, E>();
  let version = 0;
  const listeners = new Set<() => void>();
  return {
    subscribe(cb: () => void) {
      listeners.add(cb);
      return () => {
        listeners.delete(cb);
      };
    },
    getVersion: () => version,
    getContainers: () => entries as ReadonlyMap<string, E>,
    set(id: string, entry: E) {
      entries.set(id, entry);
      version++;
      listeners.forEach((cb) => cb());
    },
  };
}

const dbRegistry = makeFakeRegistry<{ block: { from: number } }>();
const httpRegistry = makeFakeRegistry<{ block: { from: number } }>();

vi.mock("@/lib/codemirror/cm-db-block", () => ({
  subscribeToDbPortals: (cb: () => void) => dbRegistry.subscribe(cb),
  getDbPortalVersion: () => dbRegistry.getVersion(),
  getDbWidgetContainers: () => dbRegistry.getContainers(),
}));
vi.mock("@/lib/codemirror/cm-http-block", () => ({
  subscribeToHttpPortals: (cb: () => void) => httpRegistry.subscribe(cb),
  getHttpPortalVersion: () => httpRegistry.getVersion(),
  getHttpWidgetContainers: () => httpRegistry.getContainers(),
}));

import { blockPortals } from "@/lib/blocks/block-portal-registry";

const fakeView = {} as unknown as EditorView;

describe("blockPortals", () => {
  it("registers exactly two portal entries, ids matching block-registry", () => {
    expect(blockPortals.map((p) => p.id)).toEqual(["db", "http"]);
  });

  it("each entry exposes a renderPortal(view, filePath) function", () => {
    for (const p of blockPortals) {
      expect(typeof p.renderPortal).toBe("function");
    }
  });

  it("DB renderPortal mounts the (mocked) DbFencedPanel for every registered DB entry", () => {
    dbRegistry.set("db_idx_0", { block: { from: 0 } });
    const db = blockPortals.find((p) => p.id === "db")!;
    const { getByTestId } = render(db.renderPortal(fakeView, "x.md"));
    expect(getByTestId("db-panel-db_idx_0")).toBeInTheDocument();
  });

  it("HTTP renderPortal mounts the (mocked) HttpFencedPanel for every registered HTTP entry", () => {
    httpRegistry.set("http_idx_0", { block: { from: 0 } });
    const http = blockPortals.find((p) => p.id === "http")!;
    const { getByTestId } = render(http.renderPortal(fakeView, "x.md"));
    expect(getByTestId("http-panel-http_idx_0")).toBeInTheDocument();
  });

  it("DB and HTTP renderPortals isolate their registries (no cross-contamination)", () => {
    // Both registries already have one entry each from prior tests; render
    // both portals and confirm the DB tree shows ONLY db panels and vice
    // versa.
    const db = blockPortals.find((p) => p.id === "db")!;
    const http = blockPortals.find((p) => p.id === "http")!;
    const dbRender = render(db.renderPortal(fakeView, "x.md"));
    expect(dbRender.queryByTestId("http-panel-http_idx_0")).toBeNull();
    dbRender.unmount();
    const httpRender = render(http.renderPortal(fakeView, "x.md"));
    expect(httpRender.queryByTestId("db-panel-db_idx_0")).toBeNull();
  });
});
