// Canvas §5 — left sidebar for the Connections refined page.
// 220px wide, three sections:
//   1. Kind filter list (9 rows, click → filter the list panel).
//   2. POR AMBIENTE — env name + dot color + count.
//   3. Hint card "Credenciais locais — Senhas vivem no keychain.
//      Conexão é só nome + host."
//
// Pure presentational: takes counts maps + selection callbacks. The
// per-vault counting + env-presence aggregation lives in the
// consumer (ConnectionsPage) so this component stays test-light.

import { Box, Stack, Flex, Text } from "@chakra-ui/react";
import { LuKey } from "react-icons/lu";

import {
  MASTER_DETAIL_SIDEBAR_WIDTH,
  MasterDetailSidebarRow,
  SectionLabel,
  SidebarHintCard,
} from "@/components/layout/shared";

import { ConnectionKindIcon } from "./ConnectionKindIcon";
import {
  CONNECTION_KIND_ORDER,
  CONNECTION_KINDS,
  type ConnectionKind,
} from "./connection-kinds";

export interface EnvSummary {
  name: string;
  /** Status dot intent: ok | warn | err. Drives the dot color. */
  status: "ok" | "warn" | "err";
  count: number;
}

export interface ConnectionsKindSidebarProps {
  /** Total connection count per kind. Kinds with 0 still render —
   * canvas spec shows the full menu. */
  countsByKind: Partial<Record<ConnectionKind, number>>;
  /** Currently-selected kind filter, or `null` for "all". */
  selectedKind: ConnectionKind | null;
  /** Click on a kind row → caller flips/clears the filter. */
  onSelectKind: (kind: ConnectionKind | null) => void;
  /** Per-environment summary for the lower section. */
  envs: EnvSummary[];
}

const STATUS_DOT_COLORS = {
  ok: "green.solid",
  warn: "yellow.solid",
  err: "red.solid",
} as const;

export function ConnectionsKindSidebar({
  countsByKind,
  selectedKind,
  onSelectKind,
  envs,
}: ConnectionsKindSidebarProps) {
  return (
    <Box
      data-testid="connections-kind-sidebar"
      w={MASTER_DETAIL_SIDEBAR_WIDTH}
      h="full"
      borderRightWidth="1px"
      borderRightColor="border"
      bg="bg.subtle"
      overflowY="auto"
      p={3}
    >
      <Stack gap={4} align="stretch">
        <Box>
          <SectionLabel mb={2}>Kind</SectionLabel>
          <Stack gap={0.5} align="stretch">
            {CONNECTION_KIND_ORDER.map((kind) => {
              const meta = CONNECTION_KINDS[kind];
              const count = countsByKind[kind] ?? 0;
              const selected = selectedKind === kind;
              return (
                <MasterDetailSidebarRow
                  key={kind}
                  testId={`kind-row-${kind}`}
                  iconSlot={<ConnectionKindIcon kind={kind} size={18} />}
                  label={meta.label}
                  count={count}
                  selected={selected}
                  onClick={() => onSelectKind(selected ? null : kind)}
                />
              );
            })}
          </Stack>
        </Box>

        <Box>
          <SectionLabel mb={2}>By environment</SectionLabel>
          {envs.length === 0 ? (
            <Text fontSize="12px" color="fg.subtle" px={2}>
              No environments
            </Text>
          ) : (
            <Stack gap={0.5} align="stretch">
              {envs.map((e) => (
                <Flex
                  key={e.name}
                  data-testid={`env-row-${e.name}`}
                  align="center"
                  gap={2}
                  px={2}
                  py="6px"
                >
                  <Box
                    h="8px"
                    w="8px"
                    borderRadius="full"
                    bg={STATUS_DOT_COLORS[e.status]}
                    flexShrink={0}
                    aria-hidden
                  />
                  <Text
                    flex={1}
                    fontFamily="mono"
                    fontSize="12px"
                    color="fg"
                  >
                    {e.name}
                  </Text>
                  <Text
                    fontFamily="mono"
                    fontSize="11px"
                    color="fg.subtle"
                    minW="22px"
                    textAlign="right"
                  >
                    {e.count}
                  </Text>
                </Flex>
              ))}
            </Stack>
          )}
        </Box>

        <Box mt="auto">
          <SidebarHintCard
            icon={LuKey}
            title="Local credentials"
            testId="connections-keychain-hint"
          >
            Passwords live in the keychain. Connection is just name + host.
          </SidebarHintCard>
        </Box>
      </Stack>
    </Box>
  );
}
