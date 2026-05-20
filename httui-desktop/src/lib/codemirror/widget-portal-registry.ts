/**
 * Generic CM6 ↔ React widget portal registry (A3 — collapses the
 * hand-rolled-per-type registry that was duplicated 1:1 between
 * `cm-http-block.tsx` and `cm-db-block.tsx`, audit 03 #7).
 *
 * One `WidgetPortalRegistry` instance per block type stays a module-level
 * singleton in the type's extension file (so the two registries remain
 * fully independent — no cross-editor / cross-type contamination); only
 * the *machinery* is shared here.
 *
 * Genuine divergences kept injectable (RULE 4 — not forced into one path):
 *  - `idPrefix`           — `http_idx_` vs `db_idx_`.
 *  - `metaChanged`        — HTTP compares alias/timeout/display/mode; DB
 *                           compares dialect/alias/connection/limit/…
 *  - `bodyChangePolicy`   — HTTP notifies immediately on body change (the
 *                           250ms debounce created a visible hole in the
 *                           form view); DB still debounces (no form view).
 *  - `dedupeSameSlotElement` — HTTP skips notify when the same DOM element
 *                           re-registers (avoids reanimating form inputs);
 *                           DB has no form view and never short-circuited.
 *  - `slots`              — HTTP has a `form` slot; DB does not.
 */

import { EditorView, WidgetType } from "@codemirror/view";

// ───── Typed ResizeObserver stash (replaces the duplicated `as any`
// `(dom as any).__cmWidgetResizeObserver` — audit 03 #8) ─────

const widgetObservers = new WeakMap<HTMLElement, ResizeObserver>();

// ───── Generic block / entry shapes ─────

/** Positional skeleton every fenced block scanner produces. */
export interface FencedBlockBase {
  from: number;
  to: number;
  info: string;
  openLineFrom: number;
  openLineTo: number;
  bodyFrom: number;
  bodyTo: number;
  closeLineFrom: number;
  closeLineTo: number;
  body: string;
}

/** Minimal shape `eq` needs across two instances of a generated widget. */
interface BlockIdWidget {
  blockId: string;
}

/**
 * Registry entry. Slots are exposed as direct optional fields
 * (`entry.toolbar`, `entry.result`, …) — the React panels read them that
 * way, so the shape is preserved exactly via a mapped type.
 */
export type PortalEntryOf<
  Slot extends string,
  Actions extends object,
  Block,
> = {
  blockId: string;
  block: Block;
  actions: Actions;
} & { [K in Slot]?: HTMLElement };

interface RegistryOptions<Slot extends string, Block extends FencedBlockBase> {
  /** `http_idx_` / `db_idx_` — index-based so metadata edits don't swap id. */
  idPrefix: string;
  /** All slots a block of this type can register (drives empty-entry GC). */
  slots: readonly Slot[];
  /** True when a metadata delta is something the React panel renders. */
  metaChanged: (prev: Block, next: Block) => boolean;
  /** How a body-only change propagates to subscribers. */
  bodyChangePolicy: "immediate" | "debounced";
  /** Skip notify when the identical DOM element re-registers (HTTP form). */
  dedupeSameSlotElement: boolean;
}

/** Decoration item used by the shared fence-decoration skeleton. */
export interface DecoItem {
  from: number;
  to: number;
  // Decoration — kept structural to avoid importing the type here.
  deco: import("@codemirror/view").Decoration;
  order: number;
}

export class WidgetPortalRegistry<
  Slot extends string,
  Actions extends object,
  Block extends FencedBlockBase & { metadata: unknown },
> {
  private readonly entries = new Map<
    string,
    PortalEntryOf<Slot, Actions, Block>
  >();
  private readonly listeners = new Set<() => void>();
  private portalVersion = 0;
  private bodyNotifyTimer: ReturnType<typeof setTimeout> | null = null;
  private readonly heightCache = new Map<string, number>();

  constructor(private readonly opts: RegistryOptions<Slot, Block>) {}

  // ── core notify / subscribe ──

  private notify(): void {
    this.portalVersion++;
    for (const fn of this.listeners) fn();
  }

  // Body-only changes (fast keystrokes in the body) trigger a debounced
  // notify so body-dependent effects still catch up without re-rendering
  // on every character. 250ms matches "idle typing".
  private scheduleBodyNotify(): void {
    if (this.bodyNotifyTimer !== null) clearTimeout(this.bodyNotifyTimer);
    this.bodyNotifyTimer = setTimeout(() => {
      this.bodyNotifyTimer = null;
      this.notify();
    }, 250);
  }

  readonly subscribe = (cb: () => void): (() => void) => {
    this.listeners.add(cb);
    return () => {
      this.listeners.delete(cb);
    };
  };

  readonly getVersion = (): number => this.portalVersion;

  readonly getContainers = (): ReadonlyMap<
    string,
    PortalEntryOf<Slot, Actions, Block>
  > => this.entries;

  /**
   * Set or update the run/cancel callbacks for a block. Called by the
   * React panel when it mounts; the CM6 keymap reads these to dispatch
   * actions without an event bus.
   */
  readonly setBlockActions = (
    blockId: string,
    actions: Partial<Actions>,
  ): void => {
    const entry = this.entries.get(blockId);
    if (!entry) return;
    entry.actions = { ...entry.actions, ...actions };
  };

  // ── slot (un)register ──

  registerSlot(
    blockId: string,
    block: Block,
    slot: Slot,
    element: HTMLElement,
  ): void {
    const prev = this.entries.get(blockId);
    if (this.opts.dedupeSameSlotElement && prev && prev[slot] === element) {
      // Same DOM container, just refreshing the widget after a doc change.
      // The block content is kept in sync by `syncBlocks` (which mutates
      // positions in place and only swaps `entry.block` when metadata or
      // body actually changed). Skipping `notify()` here avoids a
      // re-render cascade on every keystroke that would reanimate the
      // CodeMirror inputs in the form view, producing a visible flash.
      return;
    }
    const next: PortalEntryOf<Slot, Actions, Block> = prev
      ? { ...prev, block, [slot]: element }
      : ({
          blockId,
          block,
          actions: {} as Actions,
          [slot]: element,
        } as PortalEntryOf<Slot, Actions, Block>);
    this.entries.set(blockId, next);
    this.notify();
  }

  unregisterSlot(blockId: string, slot: Slot): void {
    const prev = this.entries.get(blockId);
    if (!prev) return;
    const next: PortalEntryOf<Slot, Actions, Block> = {
      ...prev,
      [slot]: undefined,
    };
    const anySlotFilled = this.opts.slots.some((s) => next[s] != null);
    if (!anySlotFilled) {
      this.entries.delete(blockId);
    } else {
      this.entries.set(blockId, next);
    }
    this.notify();
  }

  /**
   * Index-based id so metadata edits (especially the alias) don't swap the
   * id under the React panel while the user edits. Insert-above reordering
   * migrates state, a lesser evil than losing drawer focus per keystroke.
   */
  blockIdOf(_block: Block, index: number): string {
    return `${this.opts.idPrefix}${index}`;
  }

  /**
   * Keep each registry entry's `block` in sync with the latest scan, but
   * only swap to a new reference (and `notify()`) when something the React
   * panel renders changed. Position-only shifts (every keystroke anywhere
   * in the doc) mutate the existing block object in place so closures read
   * fresh coordinates while `React.memo` still sees a stable prop ref.
   */
  syncBlocks(blocks: Block[]): void {
    let meaningfulChange = false;
    for (let i = 0; i < blocks.length; i++) {
      const id = this.blockIdOf(blocks[i], i);
      const entry = this.entries.get(id);
      if (!entry) continue;
      const prev = entry.block;
      const fresh = blocks[i];
      if (prev === fresh) continue;

      const metaChanged = this.opts.metaChanged(prev, fresh);
      const bodyChanged = prev.body !== fresh.body;

      if (metaChanged) {
        entry.block = fresh;
        meaningfulChange = true;
      } else if (bodyChanged) {
        // Swap the ref so the panel eventually sees the new body. HTTP
        // notifies immediately (a pending form row promoted to committed
        // would visibly disappear during a debounce); DB debounces.
        entry.block = fresh;
        if (this.opts.bodyChangePolicy === "immediate") {
          meaningfulChange = true;
        } else {
          this.scheduleBodyNotify();
        }
      } else {
        // Same meta + same body — pure position shift from edits elsewhere.
        // Mutate in place; keep the reference stable so React.memo skips.
        // (`lang`, where present, only changes with the fence text, which
        // also changes metadata → handled by the metaChanged branch; a
        // position-only shift never alters it.)
        prev.from = fresh.from;
        prev.to = fresh.to;
        prev.bodyFrom = fresh.bodyFrom;
        prev.bodyTo = fresh.bodyTo;
        prev.openLineFrom = fresh.openLineFrom;
        prev.openLineTo = fresh.openLineTo;
        prev.closeLineFrom = fresh.closeLineFrom;
        prev.closeLineTo = fresh.closeLineTo;
        prev.info = fresh.info;
      }
    }
    if (meaningfulChange) this.notify();
  }

  // ── widget height caching ──

  private cacheKey(blockId: string, slot: Slot): string {
    return `${blockId}:${slot}`;
  }

  /**
   * Seed with `offsetHeight` (border-box incl. padding + border) so the
   * cached value matches what CM6 measures via `getBoundingClientRect`
   * when laying out block widgets. Skip the seed when `offsetHeight` is 0
   * (toDOM ran before CM6 attached the widget) — caching 0 would poison
   * `estimatedHeight` since `0 ?? fallback` keeps 0.
   */
  observeWidgetHeight(
    dom: HTMLElement,
    blockId: string,
    slot: Slot,
    view: EditorView,
  ): void {
    if (typeof ResizeObserver === "undefined") return;
    const key = this.cacheKey(blockId, slot);
    const seed = dom.offsetHeight;
    if (seed > 0) this.heightCache.set(key, seed);
    const ro = new ResizeObserver(() => {
      const prev = this.heightCache.get(key);
      const next = dom.offsetHeight;
      if (next > 0 && prev !== next) {
        this.heightCache.set(key, next);
        // Re-measure so CM6's block info reflects the new height —
        // otherwise `moveVertically` keeps a stale estimate and cursor
        // navigation + click-to-position drift.
        view.requestMeasure();
      }
    });
    ro.observe(dom);
    widgetObservers.set(dom, ro);
  }

  disconnectWidgetObserver(
    dom: HTMLElement | undefined,
    blockId: string,
    slot: Slot,
  ): void {
    if (dom) {
      widgetObservers.get(dom)?.disconnect();
      widgetObservers.delete(dom);
    }
    this.heightCache.delete(this.cacheKey(blockId, slot));
  }

  private estimatedHeight(
    blockId: string,
    slot: Slot,
    fallback: number,
  ): number {
    return this.heightCache.get(this.cacheKey(blockId, slot)) ?? fallback;
  }

  // ── generic widget factories ──

  /**
   * Single-slot register-only widget (toolbar / form). Creates a
   * `contentEditable=false` div, registers it under `slot`, observes its
   * height, and re-registers on `updateDOM`. `eq` compares blockId only so
   * body/position changes don't re-mount (which would lose React state).
   */
  slotWidget(
    slot: Slot,
    className: string,
    fallbackHeight: number,
  ): new (blockId: string, block: Block) => WidgetType {
    // Pin the outer registry inside the returned class. WidgetType
    // overrides are real methods (not arrow-properties), so they can't
    // capture the registry via `this`; aliasing is the standard pattern.
    // eslint-disable-next-line @typescript-eslint/no-this-alias
    const reg = this;
    return class SlotPortalWidget extends WidgetType {
      constructor(
        readonly blockId: string,
        readonly block: Block,
      ) {
        super();
      }
      toDOM(view: EditorView): HTMLElement {
        const div = document.createElement("div");
        div.className = className;
        div.contentEditable = "false";
        reg.registerSlot(this.blockId, this.block, slot, div);
        reg.observeWidgetHeight(div, this.blockId, slot, view);
        return div;
      }
      updateDOM(dom: HTMLElement): boolean {
        reg.registerSlot(this.blockId, this.block, slot, dom);
        return true;
      }
      destroy(dom: HTMLElement): void {
        reg.disconnectWidgetObserver(dom, this.blockId, slot);
        reg.unregisterSlot(this.blockId, slot);
      }
      eq(other: WidgetType): boolean {
        return this.blockId === (other as unknown as BlockIdWidget).blockId;
      }
      get estimatedHeight(): number {
        return reg.estimatedHeight(this.blockId, slot, fallbackHeight);
      }
      ignoreEvent(): boolean {
        return true;
      }
    };
  }

  /**
   * Close-panel widget: one block widget replacing the close-fence line
   * with fence-spacer + result portal + statusbar portal. A single big
   * block widget between two Text lines is the shape CM6's vertical-motion
   * math expects — chaining separate block widgets made arrow-up teleport
   * to line 1.
   */
  closePanelWidget(opts: {
    wrapClass: string;
    spacerClass: string;
    resultClass: string;
    statusClass: string;
    resultSlot: Slot;
    statusSlot: Slot;
    fallbackHeight: number;
  }): new (blockId: string, block: Block) => WidgetType {
    // See `slotWidget` above — same rationale for the `this` alias.
    // eslint-disable-next-line @typescript-eslint/no-this-alias
    const reg = this;
    return class ClosePanelPortalWidget extends WidgetType {
      constructor(
        readonly blockId: string,
        readonly block: Block,
      ) {
        super();
      }
      toDOM(view: EditorView): HTMLElement {
        const wrap = document.createElement("div");
        wrap.className = opts.wrapClass;
        wrap.contentEditable = "false";

        const spacer = document.createElement("div");
        spacer.className = opts.spacerClass;
        wrap.appendChild(spacer);

        const result = document.createElement("div");
        result.className = opts.resultClass;
        reg.registerSlot(this.blockId, this.block, opts.resultSlot, result);
        wrap.appendChild(result);

        const status = document.createElement("div");
        status.className = opts.statusClass;
        reg.registerSlot(this.blockId, this.block, opts.statusSlot, status);
        wrap.appendChild(status);

        reg.observeWidgetHeight(wrap, this.blockId, opts.resultSlot, view);
        return wrap;
      }
      updateDOM(dom: HTMLElement): boolean {
        const result = dom.querySelector(`.${opts.resultClass}`);
        const status = dom.querySelector(`.${opts.statusClass}`);
        if (result instanceof HTMLElement) {
          reg.registerSlot(this.blockId, this.block, opts.resultSlot, result);
        }
        if (status instanceof HTMLElement) {
          reg.registerSlot(this.blockId, this.block, opts.statusSlot, status);
        }
        return true;
      }
      destroy(dom: HTMLElement): void {
        reg.disconnectWidgetObserver(dom, this.blockId, opts.resultSlot);
        reg.unregisterSlot(this.blockId, opts.resultSlot);
        reg.unregisterSlot(this.blockId, opts.statusSlot);
      }
      eq(other: WidgetType): boolean {
        return this.blockId === (other as unknown as BlockIdWidget).blockId;
      }
      get estimatedHeight(): number {
        return reg.estimatedHeight(
          this.blockId,
          opts.resultSlot,
          opts.fallbackHeight,
        );
      }
      ignoreEvent(): boolean {
        return true;
      }
    };
  }
}
