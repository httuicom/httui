// Canvas §6 Environments — page composition.
//
// Header (serif H1 + "+ New environment" button) + grid of cards.
// Pure presentational; the consumer plugs the env list, the active
// switch handler, and the create-form slot.

import { Box, Flex, Grid, Popover, Portal, Text } from "@chakra-ui/react";
import { useCallback, type ReactNode } from "react";

import { Btn } from "@/components/atoms";

import { EnvironmentCard } from "./EnvironmentCard";
import { sortEnvironments, type EnvironmentSummary } from "./envs-meta";

export interface EnvironmentsPageProps {
  envs: ReadonlyArray<EnvironmentSummary>;
  onActivate?: (filename: string) => void;
  onCreateNew?: () => void;
  onClone?: (filename: string) => void;
  onRename?: (filename: string) => void;
  onDelete?: (filename: string) => void;
  /** Slot for the inline "+ New environment" form. */
  inlineFormSlot?: ReactNode;
  /** Form rendered as a floating popover anchored to the card whose
   * filename matches `anchoredFilename`. Outside-click + Esc fire
   * `onCloseAnchoredForm`. */
  anchoredForm?: ReactNode;
  anchoredFilename?: string | null;
  onCloseAnchoredForm?: () => void;
}

export function EnvironmentsPage({
  envs,
  onActivate,
  onCreateNew,
  onClone,
  onRename,
  onDelete,
  inlineFormSlot,
  anchoredForm,
  anchoredFilename,
  onCloseAnchoredForm,
}: EnvironmentsPageProps) {
  const sorted = sortEnvironments(envs);

  // Virtual anchor for the floating Popover — resolved on every
  // `getAnchorRect` call so the popover follows the card if it
  // moves (resize / scroll within the grid).
  const getAnchorRect = useCallback(() => {
    if (!anchoredFilename) return null;
    const card = document.querySelector<HTMLElement>(
      `[data-testid="environment-card-${anchoredFilename}"]`,
    );
    return card?.getBoundingClientRect() ?? null;
  }, [anchoredFilename]);

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
        <Popover.Root
          open={!!inlineFormSlot}
          onOpenChange={(e) => {
            if (!e.open) {
              // Close fires the create-form's own onCancel via the
              // container's `creatingEnv=false` flip — but we also
              // want clicking outside / Esc to close, so re-trigger
              // onCreateNew? No: the form lives inside the container,
              // closing it is the container's responsibility on
              // submit/cancel. Outside-click here is handled by the
              // form's own cancel ref via Popover's nested behaviour.
            }
          }}
          positioning={{ placement: "bottom-end", gutter: 8 }}
        >
          <Popover.Trigger asChild>
            <Btn
              variant="primary"
              data-testid="environments-create-new"
              onClick={onCreateNew}
              disabled={!onCreateNew}
            >
              + New environment
            </Btn>
          </Popover.Trigger>
          {inlineFormSlot ? (
            <Portal>
              <Popover.Positioner>
                <Box
                  minW="360px"
                  maxW="480px"
                  filter="drop-shadow(0 8px 24px rgba(0,0,0,0.15))"
                >
                  {inlineFormSlot}
                </Box>
              </Popover.Positioner>
            </Portal>
          ) : null}
        </Popover.Root>
      </Flex>

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
                onClone={onClone}
                onRename={onRename}
                onDelete={onDelete}
              />
            ))}
          </Grid>
        )}
      </Box>

      <Popover.Root
        open={!!anchoredForm}
        onOpenChange={(e) => {
          if (!e.open) onCloseAnchoredForm?.();
        }}
        positioning={{
          placement: "bottom-start",
          getAnchorRect,
          gutter: 8,
        }}
      >
        <Portal>
          <Popover.Positioner>
            <Box
              data-testid="environments-anchored-form"
              minW="480px"
              maxW="640px"
              filter="drop-shadow(0 8px 24px rgba(0,0,0,0.15))"
            >
              {anchoredForm}
            </Box>
          </Popover.Positioner>
        </Portal>
      </Popover.Root>
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
