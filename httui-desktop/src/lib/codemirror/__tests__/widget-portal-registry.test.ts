/**
 * Direct tests of `WidgetPortalRegistry` — covers paths not exercised
 * via the cm-http-block / cm-db-block integration tests: subscribe
 * teardown, setBlockActions on missing entry, syncBlocks branches
 * (meta-change / body-change immediate / body-change debounced /
 * position-only), blockIdOf, observe/disconnect widget height, and
 * the widget factory `eq`/`destroy` paths.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { EditorView, WidgetType } from "@codemirror/view";

import {
  WidgetPortalRegistry,
  type FencedBlockBase,
} from "@/lib/codemirror/widget-portal-registry";

interface FakeMeta {
  alias?: string;
  k?: number;
}
interface FakeBlock extends FencedBlockBase {
  metadata: FakeMeta;
}

type Slot = "toolbar" | "result" | "statusbar";
interface Actions {
  onRun?: () => void;
  onCancel?: () => void;
}

function mkBlock(opts: Partial<FakeBlock>): FakeBlock {
  return {
    from: 0,
    to: 10,
    info: "",
    openLineFrom: 0,
    openLineTo: 5,
    bodyFrom: 6,
    bodyTo: 9,
    closeLineFrom: 10,
    closeLineTo: 10,
    body: "",
    metadata: {},
    ...opts,
  };
}

type RegistryCtorOpts = ConstructorParameters<
  typeof WidgetPortalRegistry<Slot, Actions, FakeBlock>
>[0];
function mkRegistry(overrides: Partial<RegistryCtorOpts> = {}) {
  return new WidgetPortalRegistry<Slot, Actions, FakeBlock>({
    idPrefix: "fake_idx_",
    slots: ["toolbar", "result", "statusbar"],
    metaChanged: (a, b) =>
      a.metadata.alias !== b.metadata.alias || a.metadata.k !== b.metadata.k,
    bodyChangePolicy: "immediate",
    dedupeSameSlotElement: false,
    ...overrides,
  });
}

const fakeView = {
  requestMeasure: vi.fn(),
} as unknown as EditorView;

describe("WidgetPortalRegistry — subscribe / getVersion", () => {
  it("subscribe returns a teardown that removes the listener", () => {
    const r = mkRegistry();
    const cb = vi.fn();
    const unsub = r.subscribe(cb);
    r.registerSlot(
      "fake_idx_0",
      mkBlock({}),
      "toolbar",
      document.createElement("div"),
    );
    expect(cb).toHaveBeenCalledTimes(1);
    unsub();
    r.registerSlot(
      "fake_idx_1",
      mkBlock({}),
      "toolbar",
      document.createElement("div"),
    );
    // Still 1 — unsub worked.
    expect(cb).toHaveBeenCalledTimes(1);
  });

  it("getVersion increments on every notify()", () => {
    const r = mkRegistry();
    const before = r.getVersion();
    r.registerSlot(
      "fake_idx_0",
      mkBlock({}),
      "toolbar",
      document.createElement("div"),
    );
    expect(r.getVersion()).toBeGreaterThan(before);
    r.registerSlot(
      "fake_idx_1",
      mkBlock({}),
      "toolbar",
      document.createElement("div"),
    );
    expect(r.getVersion()).toBeGreaterThan(before + 1);
  });
});

describe("WidgetPortalRegistry — setBlockActions", () => {
  it("merges into existing entry's actions", () => {
    const r = mkRegistry();
    r.registerSlot(
      "fake_idx_0",
      mkBlock({}),
      "toolbar",
      document.createElement("div"),
    );
    r.setBlockActions("fake_idx_0", { onRun: vi.fn() });
    r.setBlockActions("fake_idx_0", { onCancel: vi.fn() });
    const entry = r.getContainers().get("fake_idx_0")!;
    expect(typeof entry.actions.onRun).toBe("function");
    expect(typeof entry.actions.onCancel).toBe("function");
  });

  it("is a silent no-op when the blockId has no entry yet", () => {
    const r = mkRegistry();
    expect(() =>
      r.setBlockActions("does_not_exist", { onRun: vi.fn() }),
    ).not.toThrow();
    expect(r.getContainers().has("does_not_exist")).toBe(false);
  });
});

describe("WidgetPortalRegistry — registerSlot / unregisterSlot", () => {
  it("registerSlot with dedupeSameSlotElement=true skips notify for same element", () => {
    const r = mkRegistry({ dedupeSameSlotElement: true });
    const dom = document.createElement("div");
    const cb = vi.fn();
    r.subscribe(cb);
    r.registerSlot("fake_idx_0", mkBlock({}), "toolbar", dom);
    expect(cb).toHaveBeenCalledTimes(1);
    // Re-register same element — should short-circuit.
    r.registerSlot("fake_idx_0", mkBlock({}), "toolbar", dom);
    expect(cb).toHaveBeenCalledTimes(1);
  });

  it("registerSlot with dedupeSameSlotElement=false always notifies", () => {
    const r = mkRegistry({ dedupeSameSlotElement: false });
    const dom = document.createElement("div");
    const cb = vi.fn();
    r.subscribe(cb);
    r.registerSlot("fake_idx_0", mkBlock({}), "toolbar", dom);
    r.registerSlot("fake_idx_0", mkBlock({}), "toolbar", dom);
    expect(cb).toHaveBeenCalledTimes(2);
  });

  it("unregisterSlot deletes the entry when every slot is empty", () => {
    const r = mkRegistry();
    r.registerSlot(
      "fake_idx_0",
      mkBlock({}),
      "toolbar",
      document.createElement("div"),
    );
    expect(r.getContainers().has("fake_idx_0")).toBe(true);
    r.unregisterSlot("fake_idx_0", "toolbar");
    expect(r.getContainers().has("fake_idx_0")).toBe(false);
  });

  it("unregisterSlot keeps the entry while another slot is still filled", () => {
    const r = mkRegistry();
    const b = mkBlock({});
    r.registerSlot("fake_idx_0", b, "toolbar", document.createElement("div"));
    r.registerSlot("fake_idx_0", b, "result", document.createElement("div"));
    r.unregisterSlot("fake_idx_0", "toolbar");
    const entry = r.getContainers().get("fake_idx_0");
    expect(entry).toBeDefined();
    expect(entry!.toolbar).toBeUndefined();
    expect(entry!.result).toBeInstanceOf(HTMLElement);
  });

  it("unregisterSlot is a no-op for an unknown blockId", () => {
    const r = mkRegistry();
    expect(() => r.unregisterSlot("missing", "toolbar")).not.toThrow();
  });
});

describe("WidgetPortalRegistry — blockIdOf", () => {
  it("formats id from prefix + index", () => {
    const r = mkRegistry({ idPrefix: "x_idx_" });
    expect(r.blockIdOf(mkBlock({}), 0)).toBe("x_idx_0");
    expect(r.blockIdOf(mkBlock({}), 42)).toBe("x_idx_42");
  });
});

describe("WidgetPortalRegistry — syncBlocks", () => {
  it("meaningful meta change swaps entry.block + notifies", () => {
    const r = mkRegistry();
    const prev = mkBlock({ metadata: { alias: "a" } });
    r.registerSlot(
      "fake_idx_0",
      prev,
      "toolbar",
      document.createElement("div"),
    );
    const beforeV = r.getVersion();
    const next = mkBlock({ metadata: { alias: "b" } });
    r.syncBlocks([next]);
    expect(r.getContainers().get("fake_idx_0")!.block).toBe(next);
    expect(r.getVersion()).toBeGreaterThan(beforeV);
  });

  it("body change with bodyChangePolicy=immediate swaps + notifies", () => {
    const r = mkRegistry({ bodyChangePolicy: "immediate" });
    const prev = mkBlock({ body: "x" });
    r.registerSlot(
      "fake_idx_0",
      prev,
      "toolbar",
      document.createElement("div"),
    );
    const beforeV = r.getVersion();
    const next = mkBlock({ body: "y" });
    r.syncBlocks([next]);
    expect(r.getContainers().get("fake_idx_0")!.block).toBe(next);
    expect(r.getVersion()).toBeGreaterThan(beforeV);
  });

  it("body change with bodyChangePolicy=debounced swaps but defers notify", () => {
    vi.useFakeTimers();
    try {
      const r = mkRegistry({ bodyChangePolicy: "debounced" });
      const prev = mkBlock({ body: "x" });
      r.registerSlot(
        "fake_idx_0",
        prev,
        "toolbar",
        document.createElement("div"),
      );
      const cb = vi.fn();
      r.subscribe(cb);
      const next = mkBlock({ body: "y" });
      r.syncBlocks([next]);
      // Block reference DID swap immediately.
      expect(r.getContainers().get("fake_idx_0")!.block).toBe(next);
      // But notify is deferred — cb has not fired yet.
      expect(cb).not.toHaveBeenCalled();
      vi.advanceTimersByTime(250);
      expect(cb).toHaveBeenCalledTimes(1);
    } finally {
      vi.useRealTimers();
    }
  });

  it("position-only shift mutates in place + does NOT notify", () => {
    const r = mkRegistry();
    const prev = mkBlock({ from: 0, to: 10 });
    r.registerSlot(
      "fake_idx_0",
      prev,
      "toolbar",
      document.createElement("div"),
    );
    const beforeV = r.getVersion();
    const next = mkBlock({ from: 5, to: 15 });
    r.syncBlocks([next]);
    // Reference is STABLE (same `prev` object), positions mutated.
    const entry = r.getContainers().get("fake_idx_0")!;
    expect(entry.block).toBe(prev);
    expect(entry.block.from).toBe(5);
    expect(entry.block.to).toBe(15);
    expect(r.getVersion()).toBe(beforeV);
  });

  it("skips entries that have no matching registry id", () => {
    const r = mkRegistry();
    const beforeV = r.getVersion();
    r.syncBlocks([mkBlock({})]);
    // No registered slot → loop continues → no notify.
    expect(r.getVersion()).toBe(beforeV);
  });

  it("skips when prev === fresh (identity-equal)", () => {
    const r = mkRegistry();
    const b = mkBlock({});
    r.registerSlot("fake_idx_0", b, "toolbar", document.createElement("div"));
    const beforeV = r.getVersion();
    r.syncBlocks([b]);
    expect(r.getVersion()).toBe(beforeV);
  });
});

describe("WidgetPortalRegistry — observe / disconnect widget height", () => {
  let resizeObservers: Array<{
    instance: ResizeObserver;
    cb: ResizeObserverCallback;
  }>;

  beforeEach(() => {
    resizeObservers = [];
    // Override the global ResizeObserver stub from setup.ts with a
    // capturing variant so we can fire callbacks.
    globalThis.ResizeObserver = class CapturingRO {
      constructor(cb: ResizeObserverCallback) {
        const inst = this as unknown as ResizeObserver;
        resizeObservers.push({ instance: inst, cb });
      }
      observe() {}
      unobserve() {}
      disconnect() {}
    } as unknown as typeof ResizeObserver;
  });

  afterEach(() => {
    // Restore the no-op stub.
    globalThis.ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    } as unknown as typeof ResizeObserver;
  });

  it("observeWidgetHeight no-ops when ResizeObserver is undefined", () => {
    const orig = globalThis.ResizeObserver;
    // @ts-expect-error — intentionally remove for the no-op branch.
    delete globalThis.ResizeObserver;
    try {
      const r = mkRegistry();
      expect(() =>
        r.observeWidgetHeight(
          document.createElement("div"),
          "fake_idx_0",
          "toolbar",
          fakeView,
        ),
      ).not.toThrow();
    } finally {
      globalThis.ResizeObserver = orig;
    }
  });

  it("observeWidgetHeight seeds the cache with offsetHeight when > 0", () => {
    const r = mkRegistry();
    const dom = document.createElement("div");
    Object.defineProperty(dom, "offsetHeight", {
      value: 88,
      configurable: true,
    });
    r.observeWidgetHeight(dom, "fake_idx_0", "toolbar", fakeView);
    // No public reader — assert via destruction (delete should
    // succeed without throwing).
    r.disconnectWidgetObserver(dom, "fake_idx_0", "toolbar");
  });

  it("ResizeObserver callback updates the cache + calls view.requestMeasure when height changes", () => {
    const r = mkRegistry();
    const dom = document.createElement("div");
    Object.defineProperty(dom, "offsetHeight", {
      value: 50,
      configurable: true,
    });
    const measure = vi.fn();
    r.observeWidgetHeight(dom, "fake_idx_0", "toolbar", {
      requestMeasure: measure,
    } as unknown as EditorView);
    // Simulate the height changing.
    Object.defineProperty(dom, "offsetHeight", {
      value: 90,
      configurable: true,
    });
    resizeObservers[0].cb([], resizeObservers[0].instance);
    expect(measure).toHaveBeenCalled();
  });

  it("ResizeObserver callback skips requestMeasure when height is unchanged", () => {
    const r = mkRegistry();
    const dom = document.createElement("div");
    Object.defineProperty(dom, "offsetHeight", {
      value: 50,
      configurable: true,
    });
    const measure = vi.fn();
    r.observeWidgetHeight(dom, "fake_idx_0", "toolbar", {
      requestMeasure: measure,
    } as unknown as EditorView);
    // Height unchanged → callback should NOT fire requestMeasure.
    resizeObservers[0].cb([], resizeObservers[0].instance);
    expect(measure).not.toHaveBeenCalled();
  });

  it("disconnectWidgetObserver tolerates undefined dom", () => {
    const r = mkRegistry();
    expect(() =>
      r.disconnectWidgetObserver(undefined, "fake_idx_0", "toolbar"),
    ).not.toThrow();
  });
});

describe("WidgetPortalRegistry — widget factories", () => {
  it("slotWidget produces a WidgetType subclass with eq by blockId", () => {
    const r = mkRegistry();
    const W = r.slotWidget("toolbar", "test-class", 44);
    const w1 = new W("fake_idx_0", mkBlock({}));
    const w2 = new W("fake_idx_0", mkBlock({}));
    const w3 = new W("fake_idx_1", mkBlock({}));
    expect(w1.eq(w2)).toBe(true);
    expect(w1.eq(w3)).toBe(false);
    expect(w1 instanceof WidgetType).toBe(true);
  });

  it("slotWidget toDOM registers the slot + observes height", () => {
    const r = mkRegistry();
    const W = r.slotWidget("toolbar", "test-class", 44);
    const w = new W("fake_idx_0", mkBlock({}));
    const dom = w.toDOM(fakeView);
    expect(dom.className).toBe("test-class");
    expect(dom.contentEditable).toBe("false");
    expect(r.getContainers().get("fake_idx_0")?.toolbar).toBe(dom);
  });

  it("slotWidget destroy unregisters + disconnects", () => {
    const r = mkRegistry();
    const W = r.slotWidget("toolbar", "x", 44);
    const w = new W("fake_idx_0", mkBlock({}));
    const dom = w.toDOM(fakeView);
    w.destroy(dom);
    expect(r.getContainers().has("fake_idx_0")).toBe(false);
  });

  it("slotWidget ignoreEvent returns true", () => {
    const r = mkRegistry();
    const W = r.slotWidget("toolbar", "x", 44);
    const w = new W("fake_idx_0", mkBlock({}));
    expect(w.ignoreEvent(new Event("click"))).toBe(true);
  });

  it("slotWidget updateDOM re-registers + returns true", () => {
    const r = mkRegistry();
    const W = r.slotWidget("toolbar", "x", 44);
    const w = new W("fake_idx_0", mkBlock({}));
    const dom = document.createElement("div");
    expect(w.updateDOM(dom, fakeView, w)).toBe(true);
    expect(r.getContainers().get("fake_idx_0")?.toolbar).toBe(dom);
  });

  it("slotWidget estimatedHeight uses fallback when cache empty", () => {
    const r = mkRegistry();
    const W = r.slotWidget("toolbar", "x", 77);
    const w = new W("fake_idx_0", mkBlock({}));
    expect(w.estimatedHeight).toBe(77);
  });

  it("closePanelWidget toDOM registers result + statusbar slots", () => {
    const r = mkRegistry();
    const W = r.closePanelWidget({
      wrapClass: "wrap",
      spacerClass: "sp",
      resultClass: "res",
      statusClass: "st",
      resultSlot: "result",
      statusSlot: "statusbar",
      fallbackHeight: 100,
    });
    const w = new W("fake_idx_0", mkBlock({}));
    const dom = w.toDOM(fakeView);
    expect(dom.className).toBe("wrap");
    expect(dom.querySelector(".sp")).not.toBeNull();
    expect(dom.querySelector(".res")).not.toBeNull();
    expect(dom.querySelector(".st")).not.toBeNull();
    const entry = r.getContainers().get("fake_idx_0")!;
    expect(entry.result).toBeInstanceOf(HTMLElement);
    expect(entry.statusbar).toBeInstanceOf(HTMLElement);
  });

  it("closePanelWidget updateDOM re-registers both child slots", () => {
    const r = mkRegistry();
    const W = r.closePanelWidget({
      wrapClass: "wrap",
      spacerClass: "sp",
      resultClass: "res",
      statusClass: "st",
      resultSlot: "result",
      statusSlot: "statusbar",
      fallbackHeight: 100,
    });
    const w = new W("fake_idx_0", mkBlock({}));
    const dom = w.toDOM(fakeView);
    const before = r.getContainers().get("fake_idx_0")!.result;
    expect(w.updateDOM(dom, fakeView, w)).toBe(true);
    const after = r.getContainers().get("fake_idx_0")!.result;
    // Same DOM element after updateDOM (the query inside finds the
    // existing children).
    expect(after).toBe(before);
  });

  it("closePanelWidget destroy unregisters both result + statusbar", () => {
    const r = mkRegistry();
    const W = r.closePanelWidget({
      wrapClass: "wrap",
      spacerClass: "sp",
      resultClass: "res",
      statusClass: "st",
      resultSlot: "result",
      statusSlot: "statusbar",
      fallbackHeight: 100,
    });
    const w = new W("fake_idx_0", mkBlock({}));
    const dom = w.toDOM(fakeView);
    w.destroy(dom);
    expect(r.getContainers().has("fake_idx_0")).toBe(false);
  });

  it("closePanelWidget eq compares blockId, ignoreEvent true", () => {
    const r = mkRegistry();
    const W = r.closePanelWidget({
      wrapClass: "w",
      spacerClass: "s",
      resultClass: "r",
      statusClass: "st",
      resultSlot: "result",
      statusSlot: "statusbar",
      fallbackHeight: 60,
    });
    const a = new W("id-1", mkBlock({}));
    const b = new W("id-1", mkBlock({}));
    const c = new W("id-2", mkBlock({}));
    expect(a.eq(b)).toBe(true);
    expect(a.eq(c)).toBe(false);
    expect(a.ignoreEvent(new Event("click"))).toBe(true);
    expect(a.estimatedHeight).toBe(60);
  });
});
