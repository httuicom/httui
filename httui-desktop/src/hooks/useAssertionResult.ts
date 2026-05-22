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
