// Shared display + execution state types used by every executable block
// shell (HttpFencedPanel, DbFencedPanel, StandaloneBlock, …). Originally
// lived alongside a TipTap `Node.create(...)` definition; with TipTap gone
// the node itself is unused, but the types are still the single source of
// truth for the block UI vocabulary.
export type DisplayMode = "input" | "output" | "split";
export type ExecutionState =
  | "idle"
  | "running"
  | "success"
  | "error"
  | "cached";
