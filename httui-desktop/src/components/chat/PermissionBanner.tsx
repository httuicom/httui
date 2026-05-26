import { useCallback, useEffect, useState } from "react";
import { Box, HStack, Text, Badge } from "@chakra-ui/react";
import { LuShield, LuCheck, LuX, LuGitCompareArrows } from "react-icons/lu";
import { useChatStore } from "@/stores/chat";
import { usePaneStore } from "@/stores/pane";
import { useWorkspace } from "@/contexts/WorkspaceContext";
import { readNote } from "@/lib/tauri/commands";

type PermissionScope = "once" | "session" | "always";

function formatToolInput(input: Record<string, unknown>): string {
  if ("command" in input) return String(input.command);
  if ("file_path" in input) return String(input.file_path);
  if ("pattern" in input)
    return `${input.pattern}${input.path ? ` in ${input.path}` : ""}`;
  return JSON.stringify(input, null, 2);
}

const scopeLabels: Record<PermissionScope, string> = {
  once: "Once",
  session: "Session",
  always: "Always",
};

function isUpdateNote(toolName: string): boolean {
  return toolName.includes("update_note");
}

function computeLineStats(
  original: string,
  proposed: string,
): { added: number; removed: number } {
  const origLines = original.split("\n");
  const propLines = proposed.split("\n");
  const origSet = new Set(origLines);
  const propSet = new Set(propLines);
  let added = 0;
  let removed = 0;
  for (const line of propLines) {
    if (!origSet.has(line)) added++;
  }
  for (const line of origLines) {
    if (!propSet.has(line)) removed++;
  }
  return { added, removed };
}

export function PermissionBanner() {
  const pendingPermission = useChatStore((s) => s.pendingPermission);
  const respondPermission = useChatStore((s) => s.respondPermission);
  const openDiffTab = usePaneStore((s) => s.openDiffTab);
  const closeDiffTab = usePaneStore((s) => s.closeDiffTab);
  const { vaultPath } = useWorkspace();
  const [scope, setScope] = useState<PermissionScope>("once");
  const [originalContent, setOriginalContent] = useState<string | null>(null);

  // Reset scope when a new permission request comes in
  useEffect(() => {
    if (pendingPermission) {
      setScope("once");
      setOriginalContent(null);

      // Pre-fetch original content for update_note
      if (isUpdateNote(pendingPermission.toolName) && vaultPath) {
        const path = String(pendingPermission.toolInput.path ?? "");
        if (path) {
          readNote(vaultPath, path)
            .then(setOriginalContent)
            .catch(() => setOriginalContent(""));
        }
      }
    }
  }, [pendingPermission?.permissionId]);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (!pendingPermission) return;
      if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        respondPermission(pendingPermission.permissionId, "allow", scope);
      } else if (e.key === "Enter" && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        respondPermission(pendingPermission.permissionId, "deny");
      }
    },
    [pendingPermission, respondPermission, scope],
  );

  useEffect(() => {
    if (!pendingPermission) return;
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [pendingPermission, handleKeyDown]);

  if (!pendingPermission) return null;

  const { permissionId, toolName, toolInput } = pendingPermission;
  const isNoteUpdate = isUpdateNote(toolName);
  const notePath = isNoteUpdate ? String(toolInput.path ?? "") : "";
  const proposedContent = isNoteUpdate ? String(toolInput.content ?? "") : "";
  const stats =
    isNoteUpdate && originalContent !== null
      ? computeLineStats(originalContent, proposedContent)
      : null;

  const handleViewDiff = () => {
    if (!vaultPath || originalContent === null) return;
    openDiffTab({
      filePath: notePath,
      vaultPath,
      permissionId,
      originalContent,
      proposedContent,
    });
  };

  // Compact banner for update_note
  if (isNoteUpdate) {
    return (
      <Box
        borderTop="1px solid"
        borderColor="orange.500/30"
        bg="orange.500/5"
        px={3}
        py={2}
      >
        <HStack gap={2}>
          <Box color="orange.400" flexShrink={0}>
            <LuShield size={14} />
          </Box>
          <Text fontWeight="medium" fontSize="xs" truncate flex={1}>
            {notePath}
          </Text>
          {stats && (
            <>
              {stats.added > 0 && (
                <Badge size="sm" colorPalette="green" variant="subtle">
                  +{stats.added}
                </Badge>
              )}
              {stats.removed > 0 && (
                <Badge size="sm" colorPalette="red" variant="subtle">
                  -{stats.removed}
                </Badge>
              )}
            </>
          )}
          <Box
            as="button"
            display="flex"
            alignItems="center"
            gap={1}
            px={2}
            py={0.5}
            rounded="md"
            fontSize="xs"
            fontWeight="medium"
            bg="brand.600"
            color="white"
            cursor="pointer"
            _hover={{ bg: "brand.700" }}
            onClick={handleViewDiff}
            opacity={originalContent === null ? 0.5 : 1}
            pointerEvents={originalContent === null ? "none" : "auto"}
          >
            <LuGitCompareArrows size={12} />
            View Diff
          </Box>
          <Box
            as="button"
            display="flex"
            alignItems="center"
            gap={1}
            px={2}
            py={0.5}
            rounded="md"
            fontSize="xs"
            fontWeight="medium"
            bg="bg.subtle"
            border="1px solid"
            borderColor="border"
            cursor="pointer"
            _hover={{ bg: "bg.emphasized" }}
            onClick={async () => {
              await respondPermission(permissionId, "deny");
              closeDiffTab(permissionId);
            }}
          >
            <LuX size={12} />
            Deny
          </Box>
          <Box
            as="button"
            display="flex"
            alignItems="center"
            gap={1}
            px={2}
            py={0.5}
            rounded="md"
            fontSize="xs"
            fontWeight="medium"
            bg="green.600"
            color="white"
            cursor="pointer"
            _hover={{ bg: "green.700" }}
            onClick={async () => {
              await respondPermission(permissionId, "allow", scope);
              closeDiffTab(permissionId);
            }}
          >
            <LuCheck size={12} />
            Allow
          </Box>
        </HStack>
        <Text fontSize="2xs" color="fg.muted" mt={1}>
          Enter = Deny · Cmd+Enter = Allow
        </Text>
      </Box>
    );
  }

  // Default banner for other tools
  return (
    <Box
      borderTop="1px solid"
      borderColor="orange.500/30"
      bg="orange.500/5"
      px={3}
      py={2}
    >
      <HStack gap={2} mb={1.5}>
        <Box color="orange.400" flexShrink={0}>
          <LuShield size={14} />
        </Box>
        <Text fontWeight="medium" fontSize="xs" flex={1} truncate>
          {toolName}
        </Text>
        <HStack gap={1} flexShrink={0}>
          <Box
            as="button"
            display="flex"
            alignItems="center"
            gap={1}
            px={2}
            py={0.5}
            rounded="md"
            fontSize="xs"
            fontWeight="medium"
            bg="bg.subtle"
            border="1px solid"
            borderColor="border"
            cursor="pointer"
            _hover={{ bg: "bg.emphasized" }}
            onClick={() => respondPermission(permissionId, "deny")}
          >
            <LuX size={12} />
            Deny
          </Box>
          <Box
            as="button"
            display="flex"
            alignItems="center"
            gap={1}
            px={2}
            py={0.5}
            rounded="md"
            fontSize="xs"
            fontWeight="medium"
            bg="green.600"
            color="white"
            cursor="pointer"
            _hover={{ bg: "green.700" }}
            onClick={() => respondPermission(permissionId, "allow", scope)}
          >
            <LuCheck size={12} />
            Allow
          </Box>
        </HStack>
      </HStack>

      <Box
        bg="bg.subtle"
        border="1px solid"
        borderColor="border"
        rounded="md"
        px={2}
        py={1.5}
        maxH="100px"
        overflowX="auto"
        overflowY="auto"
      >
        <Text fontFamily="mono" fontSize="2xs" whiteSpace="pre">
          {formatToolInput(toolInput)}
        </Text>
      </Box>

      {/* Scope selector */}
      <HStack gap={0} mt={1.5} mb={0.5}>
        {(["once", "session", "always"] as PermissionScope[]).map((s) => (
          <Box
            key={s}
            as="button"
            px={2}
            py={0.5}
            fontSize="2xs"
            fontWeight={scope === s ? "semibold" : "normal"}
            color={scope === s ? "fg" : "fg.muted"}
            bg={scope === s ? "bg.emphasized" : "transparent"}
            border="1px solid"
            borderColor={scope === s ? "border" : "transparent"}
            rounded="sm"
            cursor="pointer"
            _hover={{ bg: "bg.subtle" }}
            onClick={() => setScope(s)}
          >
            {scopeLabels[s]}
          </Box>
        ))}
      </HStack>

      <Text fontSize="2xs" color="fg.muted">
        Enter = Deny · Cmd+Enter = Allow ({scopeLabels[scope]})
      </Text>
    </Box>
  );
}
