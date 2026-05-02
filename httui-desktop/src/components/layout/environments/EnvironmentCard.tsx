// Canvas §6 Environments — single env card (Epic 44 Story 01).
//
// Presentational. Renders the summary + chips + active pill. Click
// fires `onActivate(filename)` so the consumer can call
// `set_active_environment` Tauri command.

import { Box, Flex, Text } from "@chakra-ui/react";

import type { EnvironmentSummary } from "./envs-meta";

export interface EnvironmentCardProps {
  env: EnvironmentSummary;
  onActivate?: (filename: string) => void;
}

export function EnvironmentCard({ env, onActivate }: EnvironmentCardProps) {
  const interactive = !!onActivate;
  return (
    <Box
      as={interactive ? "button" : "div"}
      type={interactive ? "button" : undefined}
      data-testid={`environment-card-${env.filename}`}
      data-active={env.isActive || undefined}
      data-personal={env.isPersonal || undefined}
      data-temporary={env.isTemporary || undefined}
      onClick={interactive ? () => onActivate?.(env.filename) : undefined}
      borderWidth="1px"
      borderColor={env.isActive ? "accent" : "border"}
      bg="bg.muted"
      borderRadius="6px"
      px={3}
      py={2.5}
      textAlign="left"
      cursor={interactive ? "pointer" : "default"}
      _hover={
        interactive
          ? { bg: "bg.subtle", borderColor: env.isActive ? "accent" : "fg.subtle" }
          : undefined
      }
    >
      <Flex justify="space-between" align="flex-start" gap={2} mb={1.5}>
        <Text
          fontFamily="serif"
          fontSize="14px"
          fontWeight={500}
          color="fg"
          truncate
          data-testid={`environment-card-${env.filename}-name`}
        >
          {env.name}
        </Text>
        {env.isActive && (
          <Box
            data-testid={`environment-card-${env.filename}-active-pill`}
            fontFamily="mono"
            fontSize="9px"
            fontWeight="bold"
            letterSpacing="0.04em"
            color="accent.fg"
            bg="accent"
            borderRadius="999px"
            px={1.5}
            py={0.5}
          >
            ACTIVE
          </Box>
        )}
      </Flex>

      <Flex gap={3} fontFamily="mono" fontSize="11px" color="fg.muted" mb={1.5}>
        <Text data-testid={`environment-card-${env.filename}-vars`}>
          {env.varCount} {env.varCount === 1 ? "var" : "vars"}
        </Text>
        <Text data-testid={`environment-card-${env.filename}-conns`}>
          {env.connectionsUsedCount === 0
            ? "all conns"
            : `${env.connectionsUsedCount} ${
                env.connectionsUsedCount === 1 ? "conn" : "conns"
              }`}
        </Text>
      </Flex>

      <Flex gap={1} flexWrap="wrap">
        {env.isPersonal && (
          <Chip testId={`environment-card-${env.filename}-personal-chip`}>
            personal
          </Chip>
        )}
        {env.isTemporary && (
          <Chip testId={`environment-card-${env.filename}-temporary-chip`}>
            temporary
          </Chip>
        )}
      </Flex>

      {env.description && (
        <Text
          fontSize="10px"
          color="fg.subtle"
          mt={1.5}
          truncate
          title={env.description}
          data-testid={`environment-card-${env.filename}-description`}
        >
          {env.description}
        </Text>
      )}
    </Box>
  );
}

function Chip({
  children,
  testId,
}: {
  children: React.ReactNode;
  testId: string;
}) {
  return (
    <Box
      as="span"
      data-testid={testId}
      fontFamily="mono"
      fontSize="9px"
      color="fg.muted"
      bg="bg"
      borderWidth="1px"
      borderColor="border"
      borderRadius="4px"
      px={1.5}
      py={0.5}
    >
      {children}
    </Box>
  );
}
