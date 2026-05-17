import { describe, it, expect, beforeEach, vi } from "vitest";

import {
  subscribeRefPopover,
  getRefPopoverState,
  openRefPopover,
  closeRefPopover,
  resetRefPopover,
  handleRefMousedown,
  type RefPopoverState,
} from "@/lib/blocks/cm-ref-popover";

function fakeView(docLen = 100) {
  return {
    state: {
      doc: { length: docLen },
      selection: { main: { head: 5 } },
    },
    dispatch: vi.fn(),
    focus: vi.fn(),
    posAtDOM: vi.fn(() => 42),
  };
}

function mkState(over: Partial<RefPopoverState> = {}): RefPopoverState {
  return {
    rawKey: "api_base",
    rect: { left: 10, top: 20, right: 60, bottom: 32 },
    view: fakeView() as never,
    caret: 7,
    ...over,
  };
}

/** jsdom span standing in for a `.cm-reference-highlight` chip. */
function chip(text: string) {
  const span = document.createElement("span");
  span.className = "cm-reference-highlight";
  span.textContent = text;
  span.getBoundingClientRect = () =>
    ({ left: 1, top: 2, right: 3, bottom: 4 }) as DOMRect;
  return span;
}

beforeEach(() => {
  resetRefPopover();
});

describe("cm-ref-popover emitter", () => {
  it("starts with null state", () => {
    expect(getRefPopoverState()).toBeNull();
  });

  it("openRefPopover sets state and notifies subscribers", () => {
    const cb = vi.fn();
    const unsub = subscribeRefPopover(cb);
    openRefPopover(mkState());
    expect(cb).toHaveBeenCalledTimes(1);
    expect(getRefPopoverState()?.rawKey).toBe("api_base");
    unsub();
    openRefPopover(mkState({ rawKey: "x" }));
    // Unsubscribed → not called again.
    expect(cb).toHaveBeenCalledTimes(1);
  });

  it("closeRefPopover(false) clears without touching the view", () => {
    const view = fakeView();
    openRefPopover(mkState({ view: view as never }));
    closeRefPopover(false);
    expect(getRefPopoverState()).toBeNull();
    expect(view.focus).not.toHaveBeenCalled();
    expect(view.dispatch).not.toHaveBeenCalled();
  });

  it("closeRefPopover() restores caret (clamped) and focuses CM6", () => {
    const view = fakeView(3);
    openRefPopover(mkState({ view: view as never, caret: 999 }));
    closeRefPopover();
    expect(view.dispatch).toHaveBeenCalledTimes(1);
    expect(view.focus).toHaveBeenCalledTimes(1);
    expect(getRefPopoverState()).toBeNull();
  });

  it("closeRefPopover is a no-op when nothing is open", () => {
    expect(() => closeRefPopover()).not.toThrow();
  });
});

describe("handleRefMousedown", () => {
  it("ignores clicks that are not on a reference chip", () => {
    const div = document.createElement("div");
    const e = { target: div, preventDefault: vi.fn() } as unknown as MouseEvent;
    expect(handleRefMousedown(e, fakeView() as never)).toBe(false);
    expect(getRefPopoverState()).toBeNull();
  });

  it("ignores a chip whose text is not a single {{ref}}", () => {
    const span = chip("not a ref");
    const e = {
      target: span,
      preventDefault: vi.fn(),
    } as unknown as MouseEvent;
    expect(handleRefMousedown(e, fakeView() as never)).toBe(false);
  });

  it("opens the popover for a valid chip and preventDefaults", () => {
    const span = chip("{{ api_base }}");
    const prevent = vi.fn();
    const e = {
      target: span,
      preventDefault: prevent,
    } as unknown as MouseEvent;
    const view = fakeView();
    expect(handleRefMousedown(e, view as never)).toBe(true);
    expect(prevent).toHaveBeenCalled();
    const st = getRefPopoverState();
    expect(st?.rawKey).toBe("api_base");
    expect(st?.caret).toBe(42);
    expect(st?.rect).toEqual({ left: 1, top: 2, right: 3, bottom: 4 });
  });

  it("falls back to the selection head when posAtDOM throws", () => {
    const span = chip("{{token}}");
    const e = {
      target: span,
      preventDefault: vi.fn(),
    } as unknown as MouseEvent;
    const view = fakeView();
    view.posAtDOM = vi.fn(() => {
      throw new Error("detached");
    });
    handleRefMousedown(e, view as never);
    expect(getRefPopoverState()?.caret).toBe(5);
  });

  it("finds the chip via closest() when the target is a child node", () => {
    const span = chip("{{deep}}");
    const inner = document.createElement("b");
    span.appendChild(inner);
    const e = {
      target: inner,
      preventDefault: vi.fn(),
    } as unknown as MouseEvent;
    expect(handleRefMousedown(e, fakeView() as never)).toBe(true);
    expect(getRefPopoverState()?.rawKey).toBe("deep");
  });
});
