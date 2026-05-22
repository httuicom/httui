// Inline test result banner: idle / running / ok / err states.
// Pure presentational — state lifted to the consumer.

import { Box, Flex, Text, chakra } from "@chakra-ui/react";

const RetryButton = chakra("button");

export type NewConnectionTestState =
  | { kind: "idle" }
  | { kind: "running" }
  | { kind: "ok"; detail: string; latencyMs: number }
  | { kind: "err"; message: string };

export interface NewConnectionTestBannerProps {
  state: NewConnectionTestState;
  onRetry?: () => void;
}

export function NewConnectionTestBanner({
  state,
  onRetry,
}: NewConnectionTestBannerProps) {
  if (state.kind === "idle") return null;

  if (state.kind === "running") {
    return (
      <Box
        data-testid="new-connection-test-banner-running"
        borderWidth="1px"
        borderColor="border"
        borderRadius="8px"
        bg="bg.subtle"
        px="12px"
        py="10px"
        fontSize="11px"
      >
        <Flex align="center" gap={2}>
          <Box
            data-testid="dot-running"
            h="6px"
            w="6px"
            borderRadius="full"
            bg="fg.subtle"
            aria-hidden
          />
          <Text fontWeight={500} color="fg">
            Testing…
          </Text>
        </Flex>
      </Box>
    );
  }

  if (state.kind === "ok") {
    return (
      <Box
        data-testid="new-connection-test-banner-ok"
        borderWidth="1px"
        borderColor="green.muted"
        bg="green.subtle"
        color="green.fg"
        borderRadius="8px"
        px="12px"
        py="10px"
        fontSize="11px"
      >
        <Flex align="center" gap={2}>
          <Box
            data-testid="dot-ok"
            h="6px"
            w="6px"
            borderRadius="full"
            bg="green.solid"
            aria-hidden
          />
          <Text fontWeight={500}>Connection OK</Text>
          <Text fontFamily="mono" color="green.fg" truncate>
            {state.detail} · {state.latencyMs}ms
          </Text>
          <Box flex={1} />
          {onRetry && (
            <RetryButton
              type="button"
              data-testid="new-connection-test-retry"
              onClick={onRetry}
              fontSize="11px"
              color="brand.fg"
              bg="transparent"
              border="none"
              cursor="pointer"
              _hover={{ textDecoration: "underline" }}
            >
              Re-test
            </RetryButton>
          )}
        </Flex>
      </Box>
    );
  }

  return (
    <Box
      data-testid="new-connection-test-banner-err"
      borderWidth="1px"
      borderColor="red.muted"
      bg="red.subtle"
      color="red.fg"
      borderRadius="8px"
      px="12px"
      py="10px"
      fontSize="11px"
    >
      <Flex align="center" gap={2}>
        <Box
          data-testid="dot-err"
          h="6px"
          w="6px"
          borderRadius="full"
          bg="red.solid"
          aria-hidden
        />
        <Text fontWeight={500}>Failed</Text>
        <Text fontFamily="mono" color="red.fg" truncate>
          {state.message}
        </Text>
        <Box flex={1} />
        {onRetry && (
          <RetryButton
            type="button"
            data-testid="new-connection-test-retry"
            onClick={onRetry}
            fontSize="11px"
            color="brand.fg"
            bg="transparent"
            border="none"
            cursor="pointer"
            _hover={{ textDecoration: "underline" }}
          >
            Re-test
          </RetryButton>
        )}
      </Flex>
    </Box>
  );
}
