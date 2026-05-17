import { beforeEach, describe, expect, it } from "vitest";

import { useCaptureStore } from "@/stores/captureStore";

describe("useCaptureStore", () => {
  beforeEach(() => {
    useCaptureStore.setState({ values: {} });
  });

  it("starts empty", () => {
    expect(useCaptureStore.getState().values).toEqual({});
  });

  it("setBlockCaptures wraps each value in a CaptureEntry with isSecret", () => {
    useCaptureStore
      .getState()
      .setBlockCaptures("a.md", "login", { token: "t", user_id: 99 });
    const block = useCaptureStore.getState().values["a.md"]?.["login"];
    expect(block?.token).toEqual({ value: "t", isSecret: true });
    expect(block?.user_id).toEqual({ value: 99, isSecret: false });
  });

  it("coerces non-primitive values to null", () => {
    useCaptureStore.getState().setBlockCaptures("a.md", "x", {
      obj: { nested: 1 },
      arr: [1, 2],
      und: undefined,
      nil: null,
      bool: true,
    });
    const block = useCaptureStore.getState().values["a.md"]?.["x"];
    expect(block?.obj.value).toBeNull();
    expect(block?.arr.value).toBeNull();
    expect(block?.und.value).toBeNull();
    expect(block?.nil.value).toBeNull();
    expect(block?.bool.value).toBe(true);
  });

  it("setBlockCaptures replaces the alias map (no merge)", () => {
    useCaptureStore.getState().setBlockCaptures("a.md", "x", { a: 1 });
    useCaptureStore.getState().setBlockCaptures("a.md", "x", { b: 2 });
    expect(useCaptureStore.getState().values["a.md"]?.["x"]).toEqual({
      b: { value: 2, isSecret: false },
    });
  });

  it("setBlockCaptures preserves siblings under the same file", () => {
    useCaptureStore.getState().setBlockCaptures("a.md", "x", { a: 1 });
    useCaptureStore.getState().setBlockCaptures("a.md", "y", { b: 2 });
    expect(
      Object.keys(useCaptureStore.getState().values["a.md"] ?? {}),
    ).toEqual(["x", "y"]);
  });

  it("clearBlockCaptures drops the alias and the whole file when last", () => {
    useCaptureStore.getState().setBlockCaptures("a.md", "x", { a: 1 });
    useCaptureStore.getState().clearBlockCaptures("a.md", "x");
    expect(useCaptureStore.getState().values).toEqual({});
  });

  it("clearBlockCaptures keeps siblings when removing one alias", () => {
    useCaptureStore.getState().setBlockCaptures("a.md", "x", { a: 1 });
    useCaptureStore.getState().setBlockCaptures("a.md", "y", { b: 2 });
    useCaptureStore.getState().clearBlockCaptures("a.md", "x");
    expect(
      Object.keys(useCaptureStore.getState().values["a.md"] ?? {}),
    ).toEqual(["y"]);
  });

  it("clearBlockCaptures is a no-op when alias not present", () => {
    useCaptureStore.getState().setBlockCaptures("a.md", "x", { a: 1 });
    const before = useCaptureStore.getState().values;
    useCaptureStore.getState().clearBlockCaptures("a.md", "missing");
    expect(useCaptureStore.getState().values).toBe(before);
  });

  it("clearFile drops every alias for the file", () => {
    useCaptureStore.getState().setBlockCaptures("a.md", "x", { a: 1 });
    useCaptureStore.getState().setBlockCaptures("a.md", "y", { b: 2 });
    useCaptureStore.getState().setBlockCaptures("b.md", "z", { c: 3 });
    useCaptureStore.getState().clearFile("a.md");
    expect(useCaptureStore.getState().values).toEqual({
      "b.md": { z: { c: { value: 3, isSecret: false } } },
    });
  });

  it("clearFile is a no-op when file not present", () => {
    useCaptureStore.getState().setBlockCaptures("a.md", "x", { a: 1 });
    const before = useCaptureStore.getState().values;
    useCaptureStore.getState().clearFile("missing.md");
    expect(useCaptureStore.getState().values).toBe(before);
  });

  it("clearAll resets the entire store", () => {
    useCaptureStore.getState().setBlockCaptures("a.md", "x", { a: 1 });
    useCaptureStore.getState().setBlockCaptures("b.md", "y", { b: 2 });
    useCaptureStore.getState().clearAll();
    expect(useCaptureStore.getState().values).toEqual({});
  });

  it("getCapture returns the entry or undefined", () => {
    useCaptureStore.getState().setBlockCaptures("a.md", "x", { a: "v" });
    expect(useCaptureStore.getState().getCapture("a.md", "x", "a")).toEqual({
      value: "v",
      isSecret: false,
    });
    expect(
      useCaptureStore.getState().getCapture("a.md", "x", "missing"),
    ).toBeUndefined();
    expect(
      useCaptureStore.getState().getCapture("nope.md", "x", "a"),
    ).toBeUndefined();
  });

  it("getBlockCaptures returns the alias map or {}", () => {
    useCaptureStore.getState().setBlockCaptures("a.md", "x", { a: 1, b: 2 });
    const block = useCaptureStore.getState().getBlockCaptures("a.md", "x");
    expect(Object.keys(block)).toEqual(["a", "b"]);
    expect(useCaptureStore.getState().getBlockCaptures("nope.md", "x")).toEqual(
      {},
    );
  });

  // Story 03 — persistence (loadFromCacheJson / dumpForCacheJson)

  it("loadFromCacheJson hydrates the file from a valid JSON map", () => {
    const json = JSON.stringify({
      login: { user_id: 42, role: "admin" },
      profile: { handle: "alice" },
    });
    useCaptureStore.getState().loadFromCacheJson("a.md", json);
    const file = useCaptureStore.getState().values["a.md"];
    expect(file?.login?.user_id).toEqual({ value: 42, isSecret: false });
    expect(file?.login?.role).toEqual({ value: "admin", isSecret: false });
    expect(file?.profile?.handle).toEqual({ value: "alice", isSecret: false });
  });

  it("loadFromCacheJson re-derives isSecret from key name", () => {
    // The persisted shape doesn't round-trip the secret flag — it's
    // recomputed on read so the in-memory mask survives even if the
    // user opens an older cache file.
    const json = JSON.stringify({ login: { token: "tk", user_id: 1 } });
    useCaptureStore.getState().loadFromCacheJson("a.md", json);
    const block = useCaptureStore.getState().values["a.md"]?.login;
    expect(block?.token?.isSecret).toBe(true);
    expect(block?.user_id?.isSecret).toBe(false);
  });

  it("loadFromCacheJson is a no-op on invalid JSON", () => {
    useCaptureStore.getState().setBlockCaptures("a.md", "x", { a: 1 });
    const before = useCaptureStore.getState().values;
    useCaptureStore.getState().loadFromCacheJson("a.md", "not-json");
    expect(useCaptureStore.getState().values).toBe(before);
  });

  it("loadFromCacheJson is a no-op when top-level isn't an object", () => {
    useCaptureStore.getState().setBlockCaptures("a.md", "x", { a: 1 });
    const before = useCaptureStore.getState().values;
    useCaptureStore
      .getState()
      .loadFromCacheJson("a.md", JSON.stringify([1, 2]));
    expect(useCaptureStore.getState().values).toBe(before);
    useCaptureStore.getState().loadFromCacheJson("a.md", JSON.stringify(null));
    expect(useCaptureStore.getState().values).toBe(before);
    useCaptureStore.getState().loadFromCacheJson("a.md", JSON.stringify("x"));
    expect(useCaptureStore.getState().values).toBe(before);
  });

  it("loadFromCacheJson skips alias entries that aren't objects", () => {
    // Defensive: a corrupted file shouldn't blow up the hydrate; only
    // the well-shaped aliases land.
    const json = JSON.stringify({
      good: { k: "v" },
      bad_array: [1, 2],
      bad_str: "x",
      bad_null: null,
    });
    useCaptureStore.getState().loadFromCacheJson("a.md", json);
    const file = useCaptureStore.getState().values["a.md"];
    expect(Object.keys(file ?? {})).toEqual(["good"]);
    expect(file?.good?.k).toEqual({ value: "v", isSecret: false });
  });

  it("loadFromCacheJson coerces non-primitive values via the same rules", () => {
    const json = JSON.stringify({
      block: {
        obj_val: { nested: 1 },
        arr_val: [1, 2],
        nil_val: null,
        str_val: "ok",
      },
    });
    useCaptureStore.getState().loadFromCacheJson("a.md", json);
    const block = useCaptureStore.getState().values["a.md"]?.block;
    expect(block?.obj_val?.value).toBeNull();
    expect(block?.arr_val?.value).toBeNull();
    expect(block?.nil_val?.value).toBeNull();
    expect(block?.str_val?.value).toBe("ok");
  });

  it("loadFromCacheJson replaces the file's whole capture map", () => {
    useCaptureStore.getState().setBlockCaptures("a.md", "old", { k: "v" });
    useCaptureStore
      .getState()
      .loadFromCacheJson("a.md", JSON.stringify({ fresh: { k2: "v2" } }));
    const file = useCaptureStore.getState().values["a.md"];
    expect(Object.keys(file ?? {})).toEqual(["fresh"]);
  });

  it("dumpForCacheJson returns null when the file is absent", () => {
    expect(useCaptureStore.getState().dumpForCacheJson("absent.md")).toBeNull();
  });

  it("dumpForCacheJson returns null when every alias is empty after filter", () => {
    // Single secret-named entry — drops to nothing, so the consumer
    // should skip the write.
    useCaptureStore.getState().setBlockCaptures("a.md", "x", { token: "t" });
    expect(useCaptureStore.getState().dumpForCacheJson("a.md")).toBeNull();
  });

  it("dumpForCacheJson filters secrets out of the persisted JSON", () => {
    useCaptureStore
      .getState()
      .setBlockCaptures("a.md", "login", { token: "t", user_id: 7 });
    const json = useCaptureStore.getState().dumpForCacheJson("a.md");
    expect(json).not.toBeNull();
    const parsed = JSON.parse(json!);
    expect(parsed).toEqual({ login: { user_id: 7 } });
  });

  it("dumpForCacheJson preserves non-secret aliases across blocks", () => {
    useCaptureStore.getState().setBlockCaptures("a.md", "p", { id: 1 });
    useCaptureStore.getState().setBlockCaptures("a.md", "q", { name: "alice" });
    const json = useCaptureStore.getState().dumpForCacheJson("a.md");
    expect(JSON.parse(json!)).toEqual({
      p: { id: 1 },
      q: { name: "alice" },
    });
  });

  it("dump → load round-trips non-secret values", () => {
    useCaptureStore
      .getState()
      .setBlockCaptures("a.md", "x", { id: 99, name: "bob" });
    const json = useCaptureStore.getState().dumpForCacheJson("a.md")!;
    useCaptureStore.setState({ values: {} });
    useCaptureStore.getState().loadFromCacheJson("a.md", json);
    const block = useCaptureStore.getState().values["a.md"]?.x;
    expect(block?.id).toEqual({ value: 99, isSecret: false });
    expect(block?.name).toEqual({ value: "bob", isSecret: false });
  });
});
