/** Shared display + execution state types for block UI components. */
export type DisplayMode = "input" | "output" | "split";
export type ExecutionState =
  | "idle"
  | "running"
  | "success"
  | "error"
  | "cached";
