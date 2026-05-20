/**
 * Execution-state vocabulary for the fenced executable blocks
 * (HTTP / DB). One canonical union, re-exported by each block's
 * `shared.ts` so existing `import { type ExecutionState } from
 * "./shared"` sites stay unchanged.
 *
 * NOTE: this is intentionally separate from `ExecutableBlock.ts`'s
 * `ExecutionState` (`idle | cached | running | success | error`). That
 * one belongs to the standalone/diff-viewer block path and has a
 * different vocabulary (`cached`, no `cancelled`). They are different
 * domains, not duplication — merging would change a type contract.
 * See docs-llm/code-audit/01-duplication.md §4.
 */
export type ExecutionState =
  | "idle"
  | "running"
  | "success"
  | "error"
  | "cancelled";
