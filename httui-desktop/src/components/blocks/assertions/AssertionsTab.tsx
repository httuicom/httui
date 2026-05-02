// Block-result Tests tab — Epic 45 Story 04.
//
// Presentational. Lists each assertion with ✓ / ✗ icon + the original
// raw expression + actual vs expected when failed. Pure consumer of
// `AssertionResult` produced by `useAssertionResult`. The HTTP/DB
// panels mount this between Body and Raw tabs at the consumer site.

import { Box, Flex, Text } from "@chakra-ui/react";

import type {
  AssertionFailure,
  AssertionResult,
  ParsedAssertion,
} from "@/lib/blocks/assertions";

export interface AssertionsTabProps {
  /** All parsed assertions in document order. Drives the row list. */
  assertions: ReadonlyArray<ParsedAssertion>;
  /** Aggregate result from `useAssertionResult`. Null when no run yet. */
  result: AssertionResult | null;
}

export function AssertionsTab({ assertions, result }: AssertionsTabProps) {
  if (assertions.length === 0) {
    return (
      <Text
        data-testid="assertions-tab-empty"
        fontSize="11px"
        color="fg.subtle"
        px={4}
        py={3}
      >
        No assertions in this block. Add a{" "}
        <Text as="span" fontFamily="mono">
          # expect:
        </Text>{" "}
        section after the body.
      </Text>
    );
  }

  if (!result) {
    return (
      <Text
        data-testid="assertions-tab-pending"
        fontSize="11px"
        color="fg.subtle"
        px={4}
        py={3}
      >
        Run the block to evaluate {assertions.length}{" "}
        {assertions.length === 1 ? "assertion" : "assertions"}.
      </Text>
    );
  }

  const failureByLine = new Map<number, AssertionFailure>();
  for (const f of result.failures) failureByLine.set(f.line, f);

  return (
    <Box data-testid="assertions-tab" data-pass={result.pass || undefined}>
      {assertions.map((a) => {
        const failure = failureByLine.get(a.line);
        const passed = !failure;
        return (
          <Flex
            key={`${a.line}-${a.raw}`}
            data-testid={`assertions-tab-row-${a.line}`}
            data-passed={passed || undefined}
            align="flex-start"
            gap={2}
            px={4}
            py={2}
            borderBottomWidth="1px"
            borderBottomColor="border"
          >
            <Text
              as="span"
              fontSize="14px"
              color={passed ? "brand.fg" : "error"}
              flexShrink={0}
              data-testid={`assertions-tab-row-${a.line}-icon`}
            >
              {passed ? "✓" : "✗"}
            </Text>
            <Box flex={1} minW={0}>
              <Text
                fontFamily="mono"
                fontSize="11px"
                color={passed ? "fg" : "error"}
                truncate
                title={a.raw}
              >
                {a.raw}
              </Text>
              {failure && (
                <Text
                  fontSize="10px"
                  color="fg.muted"
                  mt={0.5}
                  fontFamily="mono"
                  data-testid={`assertions-tab-row-${a.line}-failure`}
                >
                  actual {formatValue(failure.actual)} · expected{" "}
                  {formatValue(failure.expected)}
                  {failure.reason ? ` — ${failure.reason}` : ""}
                </Text>
              )}
            </Box>
          </Flex>
        );
      })}
    </Box>
  );
}

function formatValue(v: unknown): string {
  if (v === undefined) return "undefined";
  if (v === null) return "null";
  if (typeof v === "string") return JSON.stringify(v);
  if (typeof v === "number" || typeof v === "boolean") return String(v);
  try {
    return JSON.stringify(v);
  } catch {
    return String(v);
  }
}
