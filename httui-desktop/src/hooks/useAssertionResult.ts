// useAssertionResult — run-time wiring.
//
// Pure-derivation hook. Memo'd on (blockBody, ctx). Returns null when:
//   - ctx is null/undefined (block hasn't run yet)
//   - the block body has no `# expect:` section (no assertions)
// Otherwise returns the aggregate `AssertionResult`. Designed to keep
// the HTTP/DB panel monoliths from growing — the panel just calls this
// hook once after a successful run and renders the badge.

import { useMemo } from "react";

import {
  evaluateAllAssertions,
  parseAllAssertions,
  type AssertionContext,
  type AssertionResult,
} from "@/lib/blocks/assertions";

export function useAssertionResult(
  blockBody: string,
  ctx: AssertionContext | null | undefined,
): AssertionResult | null {
  return useMemo(() => {
    if (!ctx) return null;
    const parsed = parseAllAssertions(blockBody);
    if (parsed.length === 0) return null;
    return evaluateAllAssertions(parsed, ctx);
  }, [blockBody, ctx]);
}
