// Module-level holder for the active editor's `forceSave` function.
//
// Why this exists: discrete UI actions (DocHeader tag edit, preflight
// check add/edit/remove, etc.) update the doc via `dispatchDocReplace`
// and would otherwise have to wait for the auto-save debounce (1s) to
// see the new state reflected in derived UI (the pill row reads from
// disk via a Tauri command). Calling `saveActiveFileNow` after the
// dispatch flushes the write immediately so the refetch lands in the
// next animation frame.
//
// Registered by `useEditorSession` on mount; cleared on unmount. The
// indirection avoids prop-drilling `forceSave` through many layers
// (AppShell → PaneContainer → PaneNode → DocHeaderedEditor → …) just
// to reach the callbacks built inside `DocHeaderWidgetPortal`.

let saver: (() => void) | null = null;

export function setActiveFileSaver(fn: (() => void) | null): void {
  saver = fn;
}

export function saveActiveFileNow(): void {
  saver?.();
}
