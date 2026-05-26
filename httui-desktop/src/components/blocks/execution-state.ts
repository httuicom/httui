// coverage:exclude file — type-only module. The single `export type`
// below compiles to an empty .js file, so v8 coverage tracking has
// nothing to instrument and the gate reports it as MISSING forever.
// The contract is enforced by `expectTypeOf` in
// `execution-state.test.ts`. Precedent: `commands.ts` / `git.ts` /
// `block-history.ts` (pure invoke wrappers — different reason, same
// "no runtime to test" outcome).
//
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
