// Click-to-open registry for the inline `{{ref}}` quick popover
// (V11 cenário 3). The CM6 extension detects a click on a
// `.cm-reference-highlight` chip and publishes the ref + its screen
// rect to a module-level emitter; `RefPopoverHost` subscribes via
// `useSyncExternalStore` and mounts a Portal+Box popover (NOT a
// Dialog — keeps CM6 focusable). Closing restores the caret + focus
// so typing continues in the editor (cenário 6).

import { EditorView } from "@codemirror/view";
import { EditorSelection, type Extension } from "@codemirror/state";

export interface RefPopoverRect {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

export interface RefPopoverState {
  /** Inner text without braces — `api_base` or `login.response.id`. */
  rawKey: string;
  /** Viewport rect of the clicked chip (popover anchor). */
  rect: RefPopoverRect;
  view: EditorView;
  /** Doc offset of the chip — restored as the caret on close. */
  caret: number;
}

let state: RefPopoverState | null = null;
const listeners = new Set<() => void>();

function emit() {
  for (const l of listeners) l();
}

/** `useSyncExternalStore` subscribe. */
export function subscribeRefPopover(cb: () => void): () => void {
  listeners.add(cb);
  return () => {
    listeners.delete(cb);
  };
}

/** `useSyncExternalStore` snapshot — stable ref until it changes. */
export function getRefPopoverState(): RefPopoverState | null {
  return state;
}

export function openRefPopover(next: RefPopoverState): void {
  state = next;
  emit();
}

/** Close the popover. When `restoreFocus` (default), put the caret
 * back on the chip and refocus CM6 so typing keeps flowing. */
export function closeRefPopover(restoreFocus = true): void {
  const prev = state;
  if (!prev) return;
  state = null;
  emit();
  if (restoreFocus) {
    const len = prev.view.state.doc.length;
    const at = Math.min(Math.max(prev.caret, 0), len);
    prev.view.dispatch({ selection: EditorSelection.cursor(at) });
    prev.view.focus();
  }
}

/** Test seam — drop state without touching a (possibly fake) view. */
export function resetRefPopover(): void {
  state = null;
  emit();
}

/** Minimal view surface the click handler needs (keeps it unit
 * testable without standing up a real EditorView). */
export interface RefClickView {
  posAtDOM: (node: Node) => number;
  state: { selection: { main: { head: number } } };
}

/** Pure mousedown handler — opens the popover when the click landed
 * on a `{{ref}}` chip. Returns true when handled (CM6 contract). */
export function handleRefMousedown(
  event: MouseEvent,
  view: RefClickView,
): boolean {
  const target = event.target as HTMLElement | null;
  const span =
    target && typeof target.closest === "function"
      ? (target.closest(".cm-reference-highlight") as HTMLElement | null)
      : null;
  if (!span) return false;
  const m = (span.textContent ?? "").match(/^\{\{\s*([^}]+?)\s*\}\}$/);
  if (!m) return false;
  const r = span.getBoundingClientRect();
  let caret: number;
  try {
    caret = view.posAtDOM(span);
  } catch {
    caret = view.state.selection.main.head;
  }
  event.preventDefault();
  openRefPopover({
    rawKey: m[1],
    rect: { left: r.left, top: r.top, right: r.right, bottom: r.bottom },
    view: view as unknown as EditorView,
    caret,
  });
  return true;
}

/** CM6 extension: mousedown on a `{{ref}}` chip opens the popover. */
export const refClickExtension: Extension = EditorView.domEventHandlers({
  mousedown: handleRefMousedown,
});
