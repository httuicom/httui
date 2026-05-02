import {
  Box,
  Flex,
  HStack,
  Text,
  Badge,
  IconButton,
  Menu,
  Portal,
  Spinner,
} from "@chakra-ui/react";
import {
  LuPlus,
  LuDatabase,
  LuPencil,
  LuTrash2,
  LuPlugZap,
  LuRefreshCw,
} from "react-icons/lu";
import { useCallback, useEffect, useState } from "react";
import type { Connection } from "@/lib/tauri/connections";
import {
  listConnections,
  deleteConnection,
  testConnection,
} from "@/lib/tauri/connections";
import { ConnectionForm } from "./ConnectionForm";

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
            return (
            <Menu.Root
              key={conn.id}
              positioning={{ placement: "bottom-start" }}
            >
              <Menu.Trigger asChild>
                <Flex
                  data-testid={`sidebar-connection-${conn.id}`}
                  data-status={ping?.status ?? "idle"}
                  data-prod={isProd ? "true" : "false"}
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
                        color="fg.3"
                      >
                        {ping.latencyMs}ms
                      </Text>
                    )}
                  </Flex>
                </Flex>
              </Menu.Trigger>
              <Portal>
                <Menu.Positioner>
                  <Menu.Content>
                    <Menu.Item
                      value="edit"
                      onSelect={() => {
                        setEditingConn(conn);
                        setShowForm(true);
                      }}
                    >
                      <LuPencil />
                      Edit
                    </Menu.Item>
                    <Menu.Item
                      value="test"
                      onSelect={() => handleTest(conn.id)}
                    >
                      <LuPlugZap />
                      Test Connection
                    </Menu.Item>
                    <Menu.Item value="refresh" onSelect={() => refresh()}>
                      <LuRefreshCw />
                      Refresh
                    </Menu.Item>
                    <Menu.Separator />
                    <Menu.Item
                      value="delete"
                      color="fg.error"
                      onSelect={() => handleDelete(conn.id)}
                    >
                      <LuTrash2 />
                      Delete
                    </Menu.Item>
                  </Menu.Content>
                </Menu.Positioner>
              </Portal>
            </Menu.Root>
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
