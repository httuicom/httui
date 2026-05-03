// Canvas §6 Variables — detail panel composer (Epic 43 Story 02).
//
// Four sections inside the 380px detail slot: header (key + scope +
// secret chip), VALUE PER ENV list (one row per env, with Show/Hide
// + Edit/Save/Cancel for secrets), is_secret toggle (slice 3), USED
// IN BLOCKS slot (slice 4 plugs the references list here). Pure
// presentational; consumer plugs `fetchSecret`, `onCommitValue`,
// `onToggleSecret`, and `confirmDemote`.

import { Box, Flex, Text } from "@chakra-ui/react";
import type { ReactNode } from "react";

import { SectionLabel } from "@/components/layout/shared";

import type { VariableRow } from "./variable-derive";
import { VariableDetailHeader } from "./VariableDetailHeader";
import { VariableSecretToggle } from "./VariableSecretToggle";
import { VariableValueRow } from "./VariableValueRow";

export interface VariableDetailContentProps {
  row: VariableRow;
  /** All envs in the vault, in display order (one row per env). */
  envNames: ReadonlyArray<string>;
  /** Async cleartext fetch (keychain) for secret values. */
  fetchSecret?: (env: string) => Promise<string | undefined>;
  /** Per-env value commit (consumer wires `EnvironmentsStore::set_var`). */
  onCommitValue?: (env: string, next: string) => void;
  /** Per-env session override (consumer wires
   * `useSessionOverrideStore.setOverride`). When omitted, the
   * Override button is hidden in every row. */
  onSetOverride?: (env: string, next: string) => void;
  /** Drop a session override for `env`. Wired by the TEMPORARY chip click. */
  onClearOverride?: (env: string) => void;
  /** Active overrides keyed by env name — drives the TEMPORARY chip. */
  overridesByEnv?: Readonly<Record<string, string>>;
  /** is_secret flip (consumer owns keychain↔TOML migration). */
  onToggleSecret?: (next: boolean) => void;
  /** Demotion confirmation gate (secret → public). */
  confirmDemote?: () => Promise<boolean>;
  /** Slice 4 plugs the used-in-blocks reference list here. */
  usedInBlocksSlot?: ReactNode;
}

export function VariableDetailContent({
  row,
  envNames,
  fetchSecret,
  onCommitValue,
  onSetOverride,
  onClearOverride,
  overridesByEnv,
  onToggleSecret,
  confirmDemote,
  usedInBlocksSlot,
}: VariableDetailContentProps) {
  return (
    <Flex
      data-testid="variable-detail-content"
      data-key={row.key}
      direction="column"
      h="full"
    >
      <VariableDetailHeader row={row} />

      <Box flex={1} overflowY="auto">
        <SectionLabel px={4} py={2}>
          VALUE PER ENV
        </SectionLabel>
        {envNames.length === 0 ? (
          <EmptyEnvsHint />
        ) : (
          envNames.map((env) => (
            // Key includes `row.key` so switching variables forces a
            // remount — the per-row reveal/edit local state would
            // otherwise leak the previous variable's cleartext.
            <VariableValueRow
              key={`${row.key}:${env}`}
              env={env}
              value={row.values[env]}
              isSecret={row.isSecret}
              fetchSecret={fetchSecret}
              onCommit={onCommitValue}
              onSetOverride={onSetOverride}
              override={overridesByEnv?.[env]}
              onClearOverride={
                onClearOverride ? () => onClearOverride(env) : undefined
              }
            />
          ))
        )}

        <VariableSecretToggle
          isSecret={row.isSecret}
          onToggle={onToggleSecret}
          confirmDemote={confirmDemote}
        />

        <SectionLabel px={4} py={2} mt={3}>
          USED IN BLOCKS
        </SectionLabel>
        {usedInBlocksSlot ?? <UsesPlaceholder usesCount={row.usesCount} />}
      </Box>
    </Flex>
  );
}

function EmptyEnvsHint() {
  return (
    <Text
      data-testid="variable-detail-empty-envs"
      fontSize="11px"
      color="fg.subtle"
      px={4}
      py={2}
    >
      No environment defined in <code>envs/*.toml</code>.
    </Text>
  );
}

function UsesPlaceholder({ usesCount }: { usesCount: number }) {
  return (
    <Text
      data-testid="variable-detail-uses-placeholder"
      fontSize="11px"
      color="fg.subtle"
      px={4}
      py={2}
    >
      {usesCount > 0
        ? `${usesCount} reference${usesCount === 1 ? "" : "s"} in the vault — list loads in slice 4.`
        : "No references found in the vault."}
    </Text>
  );
}
