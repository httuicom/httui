import { beforeEach, describe, expect, it } from "vitest";

import { useConnectionSessionOverrideStore } from "@/stores/connectionSessionOverride";

const store = () => useConnectionSessionOverrideStore.getState();

describe("useConnectionSessionOverrideStore", () => {
  beforeEach(() => {
    useConnectionSessionOverrideStore.setState({ overrides: {} });
  });

  it("starts empty", () => {
    expect(store().overrides).toEqual({});
  });

  it("setOverride stores host + port", () => {
    store().setOverride("c1", { host: "db.staging", port: 5599 });
    expect(store().getOverride("c1")).toEqual({
      host: "db.staging",
      port: 5599,
    });
  });

  it("setOverride keeps only the provided field", () => {
    store().setOverride("c1", { host: "only-host" });
    expect(store().getOverride("c1")).toEqual({ host: "only-host" });
    store().setOverride("c2", { port: 6000 });
    expect(store().getOverride("c2")).toEqual({ port: 6000 });
  });

  it("setOverride trims blank host and ignores non-finite port", () => {
    store().setOverride("c1", { host: "  ", port: Number.NaN });
    // Both fields normalize away → no override entry.
    expect(store().getOverride("c1")).toBeUndefined();
  });

  it("setOverride replaces a prior override for the same connection", () => {
    store().setOverride("c1", { host: "a", port: 1 });
    store().setOverride("c1", { host: "b" });
    expect(store().getOverride("c1")).toEqual({ host: "b" });
  });

  it("setOverride with an empty patch drops the override", () => {
    store().setOverride("c1", { host: "a" });
    store().setOverride("c1", { host: "", port: undefined });
    expect(store().getOverride("c1")).toBeUndefined();
  });

  it("clearOverride removes the entry; no-op when absent", () => {
    store().setOverride("c1", { host: "a" });
    const before = store().overrides;
    store().clearOverride("missing");
    expect(store().overrides).toBe(before);
    store().clearOverride("c1");
    expect(store().getOverride("c1")).toBeUndefined();
  });

  it("clearAll resets every override", () => {
    store().setOverride("c1", { host: "a" });
    store().setOverride("c2", { port: 2 });
    store().clearAll();
    expect(store().overrides).toEqual({});
  });

  it("preserves siblings when overriding one connection", () => {
    store().setOverride("c1", { host: "a" });
    store().setOverride("c2", { host: "b" });
    store().clearOverride("c1");
    expect(store().overrides).toEqual({ c2: { host: "b" } });
  });
});
