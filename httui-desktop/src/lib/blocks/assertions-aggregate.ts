// Run-all assertion aggregator.
//
// Pure helper: takes a per-block list of `{ blockAlias, total, result }`
// and returns the summary `<RunAllReport>` renders. spec:
// "7 blocks, 23 assertions, 22 passed, 1 failed".

import type { AssertionResult } from "./assertions";

export interface BlockAssertionRun {
  /** Block identity (alias when present, else a synthetic fallback). */
  blockAlias: string;
  /** Number of parsed assertions in this block. */
  total: number;
  /** Hook output. Null when the block has no assertions. */
  result: AssertionResult | null;
}

export interface RunAllAssertionSummary {
  /** Total blocks observed (whether or not they had assertions). */
  blocks: number;
  /** Total assertions across all blocks. */
  assertions: number;
  /** Assertions that passed. */
  passed: number;
  /** Assertions that failed. */
  failed: number;
  /** Block aliases that contained ≥1 failure, in input order. */
  failedBlocks: string[];
  /** True when every assertion passed (or there were none). */
  allPass: boolean;
}

export function aggregateAssertionResults(
  runs: ReadonlyArray<BlockAssertionRun>,
): RunAllAssertionSummary {
  let assertions = 0;
  let passed = 0;
  let failed = 0;
  const failedBlocks: string[] = [];
  for (const run of runs) {
    assertions += run.total;
    if (!run.result) continue;
    const blockFailures = run.result.failures.length;
    failed += blockFailures;
    passed += run.total - blockFailures;
    if (blockFailures > 0) failedBlocks.push(run.blockAlias);
  }
  return {
    blocks: runs.length,
    assertions,
    passed,
    failed,
    failedBlocks,
    allPass: failed === 0,
  };
}

/** Find the first block (in input order) whose assertions failed.
 * Used by run-all to short-circuit unless the user shift-clicks. */
export function firstAssertionFailureBlock(
  runs: ReadonlyArray<BlockAssertionRun>,
): string | null {
  for (const run of runs) {
    if (run.result && !run.result.pass) return run.blockAlias;
  }
  return null;
}
