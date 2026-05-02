// Canvas §6 Variables — detail panel composer (Epic 43 Story 02).
//
// Four sections inside the 380px detail slot: header (key + scope +
// secret chip), VALORES POR AMBIENTE list (one row per env, with
// Show/Hide + Edit/Save/Cancel for secrets), is_secret toggle
// (slice 3), USED IN BLOCKS slot (slice 4 plugs the references list
// here). Pure presentational; consumer plugs `fetchSecret`,
// `onCommitValue`, `onToggleSecret`, and `confirmDemote`.

import { Box, Flex, Text } from "@chakra-ui/react";
import type { ReactNode } from "react";

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
        <SectionLabel>VALORES POR AMBIENTE</SectionLabel>
        {envNames.length === 0 ? (
          <EmptyEnvsHint />
        ) : (
          envNames.map((env) => (
            <VariableValueRow
              key={env}
              env={env}
              value={row.values[env]}
              isSecret={row.isSecret}
              fetchSecret={fetchSecret}
              onCommit={onCommitValue}
            />
          ))
        )}

        <VariableSecretToggle
          isSecret={row.isSecret}
          onToggle={onToggleSecret}
          confirmDemote={confirmDemote}
        />

        <SectionLabel mt={3}>USED IN BLOCKS</SectionLabel>
        {usedInBlocksSlot ?? <UsesPlaceholder usesCount={row.usesCount} />}
      </Box>
    </Flex>
  );
}

function SectionLabel({
  children,
  ...rest
}: {
  children: ReactNode;
  [k: string]: unknown;
}) {
  return (
    <Text
      as="div"
      fontFamily="mono"
      fontSize="10px"
      fontWeight="bold"
      letterSpacing="0.06em"
      textTransform="uppercase"
      color="fg.subtle"
      px={4}
      py={2}
      {...rest}
    >
      {children}
    </Text>
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
      Nenhum ambiente definido em <code>envs/*.toml</code>.
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
        ? `${usesCount} referência${usesCount === 1 ? "" : "s"} no vault — lista carrega na slice 4.`
        : "Nenhuma referência encontrada no vault."}
    </Text>
  );
}
