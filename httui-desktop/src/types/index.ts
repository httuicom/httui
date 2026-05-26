// Shared TypeScript types
export type DisplayMode = "input" | "output" | "split";
export type ExecutionState =
  | "idle"
  | "cached"
  | "running"
  | "success"
  | "error";

export interface BlockResult {
  status: string;
  data: unknown;
  duration_ms: number;
}
