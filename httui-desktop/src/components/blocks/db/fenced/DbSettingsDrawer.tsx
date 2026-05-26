import { useEffect } from "react";
import {
  Box,
  Button,
  Flex,
  HStack,
  IconButton,
  Input,
  NativeSelectField,
  NativeSelectRoot,
  Portal,
  Text,
} from "@chakra-ui/react";
import { LuX } from "react-icons/lu";
import type { DbBlockMetadata, DbDisplayMode } from "@/lib/blocks/db-fence";
import { type Connection, updateConnection } from "@/lib/tauri/connections";

interface DbSettingsDrawerProps {
  metadata: DbBlockMetadata;
  connections: Connection[];
  /** Resolved active connection for the read-only toggle. */
  activeConnection: Connection | null;
  resolvedBindings: { placeholder: string; raw: string; value: unknown }[];
  onClose: () => void;
  onUpdate: (patch: Partial<DbBlockMetadata>) => void;
  onDelete: () => void;
  /** Callback to reflect a write-back after the user flips read-only. */
  onConnectionsChanged: (next: Connection[]) => void;
}

export function DbSettingsDrawer({
  metadata,
  connections,
  activeConnection,
  resolvedBindings,
  onClose,
  onUpdate,
  onDelete,
  onConnectionsChanged,
}: DbSettingsDrawerProps) {
  // Close on ESC
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <Portal>
      <Box
        position="fixed"
        top={0}
        right={0}
        bottom={0}
        w="320px"
        bg="bg"
        borderLeft="1px solid"
        borderColor="border"
        zIndex={1000}
        overflowY="auto"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <Flex
          px={4}
          py={3}
          borderBottom="1px solid"
          borderColor="border"
          align="center"
          justify="space-between"
        >
          <Text fontWeight="bold" fontSize="sm">
            Block settings
          </Text>
          <IconButton
            size="xs"
            variant="ghost"
            aria-label="Close"
            onClick={onClose}
          >
            <LuX />
          </IconButton>
        </Flex>

        <Box p={4} display="flex" flexDirection="column" gap={3}>
          <Box>
            <Text fontSize="xs" color="fg.muted" mb={1}>
              Alias
            </Text>
            <Input
              size="sm"
              fontFamily="mono"
              value={metadata.alias ?? ""}
              onChange={(e) => onUpdate({ alias: e.target.value || undefined })}
            />
          </Box>

          <Box>
            <Text fontSize="xs" color="fg.muted" mb={1}>
              Connection
            </Text>
            <NativeSelectRoot size="sm">
              <NativeSelectField
                value={metadata.connection ?? ""}
                onChange={(e) =>
                  onUpdate({ connection: e.target.value || undefined })
                }
              >
                <option value="">— none —</option>
                {connections.map((c) => (
                  <option key={c.id} value={c.name}>
                    {c.name} ({c.driver})
                  </option>
                ))}
              </NativeSelectField>
            </NativeSelectRoot>
          </Box>

          {activeConnection && (
            <Flex align="center" justify="space-between">
              <Box>
                <Text fontSize="xs" color="fg.muted">
                  Read-only
                </Text>
                <Text fontSize="2xs" color="fg.muted" opacity={0.7}>
                  Confirm mutations before running (per-connection)
                </Text>
              </Box>
              <Button
                size="xs"
                variant={activeConnection.is_readonly ? "solid" : "outline"}
                colorPalette={activeConnection.is_readonly ? "orange" : "gray"}
                onClick={async () => {
                  const next = !activeConnection.is_readonly;
                  try {
                    const updated = await updateConnection(
                      activeConnection.id,
                      {
                        is_readonly: next,
                      },
                    );
                    onConnectionsChanged(
                      connections.map((c) =>
                        c.id === updated.id ? updated : c,
                      ),
                    );
                  } catch {
                    /* Silently fail — toggle snaps back on next render. */
                  }
                }}
              >
                {activeConnection.is_readonly ? "RO" : "RW"}
              </Button>
            </Flex>
          )}

          <Box>
            <Text fontSize="xs" color="fg.muted" mb={1}>
              Row limit
            </Text>
            <Input
              size="sm"
              type="number"
              min={1}
              value={metadata.limit ?? ""}
              onChange={(e) => {
                const n = Number(e.target.value);
                onUpdate({
                  limit:
                    Number.isFinite(n) && n > 0 ? Math.trunc(n) : undefined,
                });
              }}
            />
          </Box>

          <Box>
            <Text fontSize="xs" color="fg.muted" mb={1}>
              Timeout (ms)
            </Text>
            <Input
              size="sm"
              type="number"
              min={1}
              value={metadata.timeoutMs ?? ""}
              onChange={(e) => {
                const n = Number(e.target.value);
                onUpdate({
                  timeoutMs:
                    Number.isFinite(n) && n > 0 ? Math.trunc(n) : undefined,
                });
              }}
            />
          </Box>

          <Box>
            <Text fontSize="xs" color="fg.muted" mb={1}>
              Display
            </Text>
            <HStack gap={2}>
              {(["input", "split", "output"] as DbDisplayMode[]).map((m) => (
                <Button
                  key={m}
                  size="xs"
                  variant={metadata.displayMode === m ? "solid" : "outline"}
                  onClick={() => onUpdate({ displayMode: m })}
                >
                  {m}
                </Button>
              ))}
            </HStack>
          </Box>

          <Box>
            <Text fontSize="xs" color="fg.muted" mb={1}>
              Resolved bindings ({resolvedBindings.length})
            </Text>
            {resolvedBindings.length === 0 ? (
              <Text fontSize="xs" color="fg.muted" opacity={0.6}>
                Run the block to see the {"{{ref}}"} → $N mapping.
              </Text>
            ) : (
              <Box
                fontFamily="mono"
                fontSize="xs"
                display="flex"
                flexDirection="column"
                gap={1}
              >
                {resolvedBindings.map((b, i) => (
                  <Flex key={i} gap={2}>
                    <Text flexShrink={0} color="fg.muted">
                      {b.placeholder}
                    </Text>
                    <Text flexShrink={0}>{b.raw}</Text>
                    <Text color="fg.muted" truncate>
                      = {JSON.stringify(b.value)}
                    </Text>
                  </Flex>
                ))}
              </Box>
            )}
          </Box>

          <Box mt={4} pt={3} borderTop="1px solid" borderColor="border">
            <Button
              size="sm"
              variant="outline"
              colorPalette="red"
              onClick={onDelete}
            >
              Delete block
            </Button>
          </Box>
        </Box>
      </Box>
    </Portal>
  );
}
