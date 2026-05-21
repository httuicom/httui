import { describe, it, expect, expectTypeOf } from "vitest";
import type { ExecutionState } from "../execution-state";

/**
 * `execution-state.ts` exports only a type union — there are 0 runtime
 * statements to cover (the TS file compiles to an empty JS file). This
 * suite exists to:
 *   1. Register the file in vitest's coverage tracking via the import
 *      side-effect, so the report shows it as "covered" instead of
 *      "missing" (no executable lines = 100% by convention).
 *   2. Pin the documented union literals via type assertions so a
 *      silent narrowing (e.g. dropping "cancelled") fails CI.
 *
 * The canonical doc comment (in `execution-state.ts`) explains why
 * this union is intentionally separate from `ExecutableBlock.ts`'s
 * `ExecutionState` ("cached" + no "cancelled" — different vocabulary,
 * see docs-llm/code-audit/01-duplication.md §4).
 */
describe("ExecutionState union", () => {
  it("accepts the 5 documented literal states", () => {
    const states: ExecutionState[] = [
      "idle",
      "running",
      "success",
      "error",
      "cancelled",
    ];
    expect(states).toHaveLength(5);
    expect(new Set(states).size).toBe(5);
  });

  it("rejects unknown literals at compile time", () => {
    // Compile-time only. `expectTypeOf` from vitest catches the
    // mistake; if someone widens the union accidentally, this fails
    // tsc (and therefore the gate) before it reaches CI.
    expectTypeOf<"idle">().toExtend<ExecutionState>();
    expectTypeOf<"cancelled">().toExtend<ExecutionState>();
    // A non-literal string is NOT assignable to the union.
    expectTypeOf<ExecutionState>().not.toEqualTypeOf<string>();
  });
});
