// Canvas §6 Environments — page composition (Epic 44 Story 01).
//
// Header (serif H1 + "+ New environment" button) + grid of cards.
// Pure presentational; the consumer plugs the env list, the active
// switch handler, and the create-form slot.

import { Box, Flex, Grid, Text } from "@chakra-ui/react";
import type { ReactNode } from "react";

import { Btn } from "@/components/atoms";

import { EnvironmentCard } from "./EnvironmentCard";
import { sortEnvironments, type EnvironmentSummary } from "./envs-meta";

export interface EnvironmentsPageProps {
  envs: ReadonlyArray<EnvironmentSummary>;
  onActivate?: (filename: string) => void;
  onCreateNew?: () => void;
  /** Slot for the inline "+ New environment" form (Story 02). */
  inlineFormSlot?: ReactNode;
}

export function EnvironmentsPage({
  envs,
  onActivate,
  onCreateNew,
  inlineFormSlot,
}: EnvironmentsPageProps) {
  const sorted = sortEnvironments(envs);

  return (
    <Flex
      data-testid="environments-page"
      direction="column"
      h="full"
      minH={0}
      overflow="hidden"
    >
      <Flex
        align="flex-end"
        justify="space-between"
        px={5}
        pt={5}
        pb={3}
        gap={3}
      >
        <Box>
          <Text fontFamily="serif" fontSize="26px" fontWeight={500} color="fg">
            Environments
          </Text>
          <Text
            fontSize="11px"
            color="fg.muted"
            data-testid="environments-page-subtitle"
          >
            workspace envs in <code>envs/*.toml</code> · personal in{" "}
            <code>*.local.toml</code> (gitignored)
          </Text>
        </Box>
        <Btn
          variant="primary"
          data-testid="environments-create-new"
          onClick={onCreateNew}
          disabled={!onCreateNew}
        >
          + New environment
        </Btn>
      </Flex>

      {inlineFormSlot}

      <Box flex={1} overflowY="auto" px={5} pb={5}>
        {sorted.length === 0 ? (
          <EmptyHint />
        ) : (
          <Grid
            data-testid="environments-grid"
            gridTemplateColumns="repeat(auto-fill, minmax(220px, 1fr))"
            gap={3}
          >
            {sorted.map((env) => (
              <EnvironmentCard
                key={env.filename}
                env={env}
                onActivate={onActivate}
              />
            ))}
          </Grid>
        )}
      </Box>
    </Flex>
  );
}

function EmptyHint() {
  return (
    <Flex
      data-testid="environments-empty-hint"
      direction="column"
      align="center"
      justify="center"
      py={10}
      gap={2}
    >
      <Text fontFamily="serif" fontSize="16px" color="fg.muted">
        No environments yet
      </Text>
      <Text fontSize="11px" color="fg.subtle" textAlign="center" maxW="420px">
        Use{" "}
        <Text as="span" fontFamily="mono">
          + New environment
        </Text>{" "}
        or create a file in{" "}
        <Text as="span" fontFamily="mono">
          envs/&lt;name&gt;.toml
        </Text>{" "}
        manually — the file watcher picks it up.
      </Text>
    </Flex>
  );
}
