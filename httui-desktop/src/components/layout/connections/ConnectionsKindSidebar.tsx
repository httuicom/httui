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

import { Box, Stack, Flex, Text, chakra } from "@chakra-ui/react";

import { ConnectionKindIcon } from "./ConnectionKindIcon";
import {
  CONNECTION_KIND_ORDER,
  CONNECTION_KINDS,
  type ConnectionKind,
} from "./connection-kinds";

const KindRowButton = chakra("button");

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
      w="220px"
      h="full"
      borderRightWidth="1px"
      borderRightColor="border"
      bg="bg.subtle"
      overflowY="auto"
      p={3}
    >
      <Stack gap={4} align="stretch">
        <Box>
          <Text
            fontFamily="mono"
            fontSize="11px"
            fontWeight="bold"
            letterSpacing="0.08em"
            textTransform="uppercase"
            color="fg.muted"
            mb={2}
          >
            Kind
          </Text>
          <Stack gap={0.5} align="stretch">
            {CONNECTION_KIND_ORDER.map((kind) => {
              const meta = CONNECTION_KINDS[kind];
              const count = countsByKind[kind] ?? 0;
              const selected = selectedKind === kind;
              return (
                <KindRowButton
                  key={kind}
                  type="button"
                  data-testid={`kind-row-${kind}`}
                  data-selected={selected ? "true" : "false"}
                  onClick={() => onSelectKind(selected ? null : kind)}
                  display="flex"
                  alignItems="center"
                  gap={2}
                  px={2}
                  py="6px"
                  borderRadius="6px"
                  bg={selected ? "bg.emphasized" : "transparent"}
                  cursor="pointer"
                  textAlign="left"
                  border="none"
                  _hover={{ bg: selected ? "bg.emphasized" : "bg.muted" }}
                >
                  <ConnectionKindIcon kind={kind} size={18} />
                  <Text
                    flex={1}
                    fontSize="13px"
                    fontWeight={selected ? 600 : 500}
                    color="fg"
                  >
                    {meta.label}
                  </Text>
                  <Text
                    fontFamily="mono"
                    fontSize="11px"
                    color="fg.subtle"
                    minW="22px"
                    textAlign="right"
                  >
                    {count}
                  </Text>
                </KindRowButton>
              );
            })}
          </Stack>
        </Box>

        <Box>
          <Text
            fontFamily="mono"
            fontSize="11px"
            fontWeight="bold"
            letterSpacing="0.08em"
            textTransform="uppercase"
            color="fg.muted"
            mb={2}
          >
            Por ambiente
          </Text>
          {envs.length === 0 ? (
            <Text fontSize="12px" color="fg.subtle" px={2}>
              Sem ambientes
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

        <Box
          data-testid="connections-keychain-hint"
          fontSize="10px"
          lineHeight={1.4}
          color="fg.muted"
          bg="bg.muted"
          borderWidth="1px"
          borderColor="border"
          borderRadius="6px"
          p={2.5}
          mt="auto"
        >
          <Text as="span" mr={1} aria-hidden>
            🔑
          </Text>
          <Text as="span" fontWeight={600}>
            Credenciais locais —
          </Text>{" "}
          Senhas vivem no keychain. Conexão é só nome + host.
        </Box>
      </Stack>
    </Box>
  );
}
