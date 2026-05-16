import {
  Box,
  Flex,
  HStack,
  Text,
  Badge,
  IconButton,
  Popover,
  Portal,
  Spinner,
} from "@chakra-ui/react";
import { LuPlus, LuDatabase } from "react-icons/lu";
import { useCallback, useEffect, useState } from "react";
import type { Connection } from "@/lib/tauri/connections";
import {
  listConnections,
  createConnection,
  deleteConnection,
  testConnection,
} from "@/lib/tauri/connections";
import { useConnectionSessionOverrideStore } from "@/stores/connectionSessionOverride";
import { TemporaryChip } from "@/components/layout/variables/TemporaryChip";
import { ConnectionForm } from "./ConnectionForm";
import { ConnectionQuickEdit } from "./ConnectionQuickEdit";

const DRIVER_LABELS: Record<string, string> = {
  postgres: "PG",
  mysql: "MY",
  sqlite: "SL",
};

const DRIVER_COLORS: Record<string, string> = {
  postgres: "blue",
  mysql: "orange",
  sqlite: "green",
};

const PROD_PATTERN = /prod/i;

interface PingState {
  status: "idle" | "ok" | "err";
  latencyMs: number | null;
}

async function pingConnection(id: string): Promise<PingState> {
  const start = performance.now();
  try {
    await testConnection(id);
    return { status: "ok", latencyMs: Math.round(performance.now() - start) };
  } catch {
    return { status: "err", latencyMs: null };
  }
}

export function ConnectionsList() {
  const [connections, setConnections] = useState<Connection[]>([]);
  const [editingConn, setEditingConn] = useState<Connection | null>(null);
  const [showForm, setShowForm] = useState(false);
  const [testing, setTesting] = useState<string | null>(null);
  const [pings, setPings] = useState<Record<string, PingState>>({});
  const overrides = useConnectionSessionOverrideStore((s) => s.overrides);

  const refresh = useCallback(async () => {
    try {
      const conns = await listConnections();
      setConnections(conns);
    } catch {
      // silently fail
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  // Auto-ping every connection on mount + whenever the connection
  // list changes. Fire-and-forget per id so a slow one doesn't block
  // the others. Failures land as `status="err"` in the ping map.
  useEffect(() => {
    let cancelled = false;
    for (const conn of connections) {
      pingConnection(conn.id).then((result) => {
        if (cancelled) return;
        setPings((prev) => ({ ...prev, [conn.id]: result }));
      });
    }
    return () => {
      cancelled = true;
    };
  }, [connections]);

  const handleDelete = useCallback(
    async (id: string) => {
      try {
        await deleteConnection(id);
        await refresh();
      } catch {
        // ignore
      }
    },
    [refresh],
  );

  const handleTest = useCallback(async (id: string) => {
    setTesting(id);
    const result = await pingConnection(id);
    setPings((prev) => ({ ...prev, [id]: result }));
    setTesting(null);
  }, []);

  const handleDuplicate = useCallback(
    async (conn: Connection) => {
      try {
        // Password lives in the keychain and can't be read back — the
        // copy starts without one (rotate it from the popover).
        await createConnection({
          name: `${conn.name} copy`,
          driver: conn.driver,
          host: conn.host ?? undefined,
          port: conn.port ?? undefined,
          database_name: conn.database_name ?? undefined,
          username: conn.username ?? undefined,
          ssl_mode: conn.ssl_mode ?? undefined,
          timeout_ms: conn.timeout_ms,
          query_timeout_ms: conn.query_timeout_ms,
          ttl_seconds: conn.ttl_seconds,
          max_pool_size: conn.max_pool_size,
          is_readonly: conn.is_readonly,
        });
        await refresh();
      } catch {
        // Name collision / backend reject — ignore (matches the
        // list's existing fire-and-forget error posture).
      }
    },
    [refresh],
  );

  const handleFormClose = useCallback(() => {
    setShowForm(false);
    setEditingConn(null);
    refresh();
  }, [refresh]);

  return (
    <>
      <HStack px={3} py={2} justify="space-between">
        <Text
          fontSize="xs"
          fontWeight="semibold"
          color="fg.subtle"
          textTransform="uppercase"
          letterSpacing="wider"
        >
          Connections
        </Text>
        <IconButton
          aria-label="New connection"
          variant="ghost"
          size="xs"
          onClick={() => {
            setEditingConn(null);
            setShowForm(true);
          }}
        >
          <LuPlus />
        </IconButton>
      </HStack>

      {connections.length === 0 ? (
        <Box px={3} py={4} textAlign="center">
          <Text fontSize="sm" color="fg.muted">
            No connections
          </Text>
        </Box>
      ) : (
        <Box px={1} pb={2}>
          {connections.map((conn) => {
            const ping = pings[conn.id];
            const isProd = PROD_PATTERN.test(conn.name);
            const hasOverride = conn.id in overrides;
            return (
            <Popover.Root
              key={conn.id}
              lazyMount
              unmountOnExit
              positioning={{ placement: "right-start", gutter: 6 }}
            >
              <Popover.Trigger asChild>
                <Flex
                  data-testid={`sidebar-connection-${conn.id}`}
                  data-status={ping?.status ?? "idle"}
                  data-prod={isProd ? "true" : "false"}
                  data-temporary={hasOverride ? "true" : "false"}
                  align="center"
                  gap={2}
                  px={2}
                  py={1}
                  mx={1}
                  rounded="md"
                  cursor="pointer"
                  _hover={{ bg: "bg.subtle" }}
                  fontSize="sm"
                >
                  <LuDatabase size={14} />
                  <Text flex={1} truncate fontFamily="mono" fontSize="xs">
                    {conn.name}
                  </Text>
                  {hasOverride && (
                    <Box flexShrink={0}>
                      <TemporaryChip />
                    </Box>
                  )}
                  {isProd && (
                    <Text
                      data-testid={`sidebar-connection-${conn.id}-prod`}
                      fontSize="2xs"
                      fontWeight={700}
                      letterSpacing="0.06em"
                      px="4px"
                      py="1px"
                      color="red.fg"
                      bg="red.subtle"
                      borderRadius="3px"
                      flexShrink={0}
                    >
                      PROD
                    </Text>
                  )}
                  <Badge
                    size="sm"
                    variant="subtle"
                    colorPalette={DRIVER_COLORS[conn.driver] ?? "gray"}
                    fontFamily="mono"
                    fontSize="2xs"
                  >
                    {DRIVER_LABELS[conn.driver] ?? conn.driver}
                  </Badge>
                  <Flex align="center" gap={1} flexShrink={0}>
                    {testing === conn.id ? (
                      <Spinner size="xs" />
                    ) : (
                      <Box
                        data-testid={`sidebar-connection-${conn.id}-dot`}
                        w={2}
                        h={2}
                        rounded="full"
                        bg={
                          ping?.status === "ok"
                            ? "green.500"
                            : ping?.status === "err"
                              ? "red.500"
                              : "gray.500"
                        }
                      />
                    )}
                    {ping?.latencyMs != null && (
                      <Text
                        data-testid={`sidebar-connection-${conn.id}-latency`}
                        fontFamily="mono"
                        fontSize="2xs"
                        color="fg.subtle"
                      >
                        {ping.latencyMs}ms
                      </Text>
                    )}
                  </Flex>
                </Flex>
              </Popover.Trigger>
              <Portal>
                <Popover.Positioner>
                  <Popover.Content
                    width="auto"
                    bg="transparent"
                    borderWidth={0}
                    boxShadow="none"
                  >
                    <ConnectionQuickEdit
                      conn={conn}
                      pingStatus={ping?.status ?? "idle"}
                      pingLatencyMs={ping?.latencyMs ?? null}
                      onTest={() => handleTest(conn.id)}
                      onEdit={() => {
                        setEditingConn(conn);
                        setShowForm(true);
                      }}
                      onDelete={() => handleDelete(conn.id)}
                      onDuplicate={() => handleDuplicate(conn)}
                      onChanged={() => refresh()}
                    />
                  </Popover.Content>
                </Popover.Positioner>
              </Portal>
            </Popover.Root>
            );
          })}
        </Box>
      )}

      {showForm && (
        <ConnectionForm connection={editingConn} onClose={handleFormClose} />
      )}
    </>
  );
}
