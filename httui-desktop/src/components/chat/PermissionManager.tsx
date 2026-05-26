import { useState, useEffect, useCallback } from "react";
import {
  Box,
  Flex,
  HStack,
  VStack,
  Text,
  IconButton,
  Badge,
  Portal,
} from "@chakra-ui/react";
import { LuTrash2, LuX, LuShield } from "react-icons/lu";
import {
  listToolPermissions,
  deleteToolPermission,
  type ToolPermission,
} from "@/lib/tauri/chat";

interface PermissionManagerProps {
  open: boolean;
  onClose: () => void;
}

function timeAgo(timestamp: number): string {
  const diff = Math.floor(Date.now() / 1000) - timestamp;
  if (diff < 60) return "just now";
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  return `${Math.floor(diff / 86400)}d ago`;
}

export function PermissionManager({ open, onClose }: PermissionManagerProps) {
  const [rules, setRules] = useState<ToolPermission[]>([]);

  const refresh = useCallback(async () => {
    try {
      const list = await listToolPermissions();
      setRules(list);
    } catch (e) {
      console.error("Failed to load permissions:", e);
    }
  }, []);

  useEffect(() => {
    if (open) refresh();
  }, [open, refresh]);

  const handleDelete = useCallback(
    async (id: number) => {
      try {
        await deleteToolPermission(id);
        await refresh();
      } catch (e) {
        console.error("Failed to delete permission:", e);
      }
    },
    [refresh],
  );

  if (!open) return null;

  // Group by workspace
  const grouped = new Map<string, ToolPermission[]>();
  for (const rule of rules) {
    const key = rule.workspace ?? "Global";
    if (!grouped.has(key)) grouped.set(key, []);
    grouped.get(key)!.push(rule);
  }

  return (
    <Portal>
      <Box
        position="fixed"
        top={0}
        right={0}
        w="360px"
        h="100vh"
        bg="bg"
        borderLeft="1px solid"
        borderColor="border"
        zIndex={1000}
        display="flex"
        flexDirection="column"
      >
        {/* Header */}
        <HStack
          px={3}
          py={2}
          borderBottom="1px solid"
          borderColor="border"
          flexShrink={0}
        >
          <LuShield size={14} />
          <Text fontWeight="semibold" fontSize="sm" flex={1}>
            Permission Rules
          </Text>
          <IconButton
            aria-label="Close"
            size="xs"
            variant="ghost"
            onClick={onClose}
          >
            <LuX />
          </IconButton>
        </HStack>

        {/* Content */}
        <Flex direction="column" flex={1} overflow="auto" p={3} gap={3}>
          {rules.length === 0 ? (
            <VStack py={8} gap={2}>
              <Text fontSize="sm" color="fg.muted">
                No saved permission rules
              </Text>
              <Text fontSize="xs" color="fg.muted">
                Rules are created when you allow tools with "Session" or
                "Always" scope
              </Text>
            </VStack>
          ) : (
            Array.from(grouped.entries()).map(([workspace, groupRules]) => (
              <Box key={workspace}>
                <Text
                  fontSize="2xs"
                  fontWeight="semibold"
                  color="fg.muted"
                  mb={1.5}
                  textTransform="uppercase"
                >
                  {workspace}
                </Text>
                <VStack gap={1} align="stretch">
                  {groupRules.map((rule) => (
                    <HStack
                      key={rule.id}
                      px={2}
                      py={1.5}
                      bg="bg.subtle"
                      border="1px solid"
                      borderColor="border"
                      rounded="md"
                      gap={2}
                      role="group"
                    >
                      <Badge
                        size="sm"
                        colorPalette={
                          rule.behavior === "allow" ? "green" : "red"
                        }
                        variant="subtle"
                      >
                        {rule.behavior}
                      </Badge>
                      <Text fontSize="xs" fontWeight="medium" flex={1}>
                        {rule.tool_name}
                      </Text>
                      <Badge size="sm" variant="outline">
                        {rule.scope}
                      </Badge>
                      <Text fontSize="2xs" color="fg.muted">
                        {timeAgo(rule.created_at)}
                      </Text>
                      <IconButton
                        aria-label="Delete rule"
                        size="2xs"
                        variant="ghost"
                        colorPalette="red"
                        opacity={0}
                        _groupHover={{ opacity: 1 }}
                        onClick={() => handleDelete(rule.id)}
                      >
                        <LuTrash2 />
                      </IconButton>
                    </HStack>
                  ))}
                </VStack>
              </Box>
            ))
          )}
        </Flex>
      </Box>
    </Portal>
  );
}
