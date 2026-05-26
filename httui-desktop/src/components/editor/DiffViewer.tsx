import { useEffect, useRef, useCallback, useState } from "react";
import { Box, HStack, Text, Badge } from "@chakra-ui/react";
import { LuCheck, LuX } from "react-icons/lu";
import { MergeView } from "@codemirror/merge";
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { useChatStore } from "@/stores/chat";
import { usePaneStore } from "@/stores/pane";
import { createBlockWidgetPlugin } from "@/lib/codemirror/cm-block-widgets.tsx";
import type { TabState } from "@/types/pane";

interface DiffViewerProps {
  tab: TabState;
}

type PermissionScope = "once" | "session" | "always";

const scopeLabels: Record<PermissionScope, string> = {
  once: "Once",
  session: "Session",
  always: "Always",
};

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

const readOnlyExtension = EditorState.readOnly.of(true);
const themeExtension = EditorView.theme({
  "&": { height: "100%", fontSize: "13px" },
  ".cm-content": { fontFamily: "var(--chakra-fonts-mono)", padding: "8px 0" },
  ".cm-gutters": { background: "transparent", borderRight: "none" },
  ".cm-line": { padding: "0 8px" },
});

export function DiffViewer({ tab }: DiffViewerProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const mergeViewRef = useRef<MergeView | null>(null);
  const pendingPermission = useChatStore((s) => s.pendingPermission);
  const respondPermission = useChatStore((s) => s.respondPermission);
  const closeDiffTab = usePaneStore((s) => s.closeDiffTab);
  const [scope, setScope] = useState<PermissionScope>("once");

  const original = tab.originalContent ?? "";
  const proposed = tab.proposedContent ?? "";
  const stats = computeLineStats(original, proposed);

  // Create MergeView
  useEffect(() => {
    if (!containerRef.current) return;

    const view = new MergeView({
      a: {
        doc: original,
        extensions: [
          readOnlyExtension,
          themeExtension,
          createBlockWidgetPlugin(proposed, "a"),
        ],
      },
      b: {
        doc: proposed,
        extensions: [
          readOnlyExtension,
          themeExtension,
          createBlockWidgetPlugin(original, "b"),
        ],
      },
      parent: containerRef.current,
      highlightChanges: true,
      gutter: true,
      collapseUnchanged: { margin: 3, minSize: 6 },
    });

    mergeViewRef.current = view;

    return () => {
      view.destroy();
      mergeViewRef.current = null;
    };
  }, [original, proposed]);

  // Auto-close when permission is resolved externally (not by this component)
  const mountedRef = useRef(false);
  useEffect(() => {
    // Skip the first render — pendingPermission is valid at mount time
    if (!mountedRef.current) {
      mountedRef.current = true;
      return;
    }
    if (!tab.permissionId) return;
    if (
      !pendingPermission ||
      pendingPermission.permissionId !== tab.permissionId
    ) {
      closeDiffTab(tab.permissionId);
    }
  }, [pendingPermission, tab.permissionId, closeDiffTab]);

  const handleAllow = useCallback(async () => {
    if (!tab.permissionId) return;
    await respondPermission(tab.permissionId, "allow", scope);
    closeDiffTab(tab.permissionId);
  }, [tab.permissionId, respondPermission, scope, closeDiffTab]);

  const handleDeny = useCallback(async () => {
    if (!tab.permissionId) return;
    await respondPermission(tab.permissionId, "deny");
    closeDiffTab(tab.permissionId);
  }, [tab.permissionId, respondPermission, closeDiffTab]);

  return (
    <Box h="100%" display="flex" flexDirection="column" overflow="hidden">
      {/* Header */}
      <HStack
        px={3}
        py={1.5}
        borderBottom="1px solid"
        borderColor="border"
        bg="bg.subtle"
        flexShrink={0}
        gap={2}
      >
        <Text fontSize="xs" fontWeight="medium" flex={1}>
          {tab.filePath}
        </Text>
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

        {/* Scope selector */}
        <HStack gap={0}>
          {(["once", "session", "always"] as PermissionScope[]).map((s) => (
            <Box
              key={s}
              as="button"
              px={1.5}
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
          onClick={handleDeny}
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
          onClick={handleAllow}
        >
          <LuCheck size={12} />
          Allow
        </Box>
      </HStack>

      {/* Labels */}
      <HStack
        gap={0}
        flexShrink={0}
        borderBottom="1px solid"
        borderColor="border"
      >
        <Box flex={1} px={3} py={1} bg="red.500/5">
          <Text fontSize="2xs" color="fg.muted" fontWeight="medium">
            Current
          </Text>
        </Box>
        <Box
          flex={1}
          px={3}
          py={1}
          bg="green.500/5"
          borderLeft="1px solid"
          borderColor="border"
        >
          <Text fontSize="2xs" color="fg.muted" fontWeight="medium">
            Proposed
          </Text>
        </Box>
      </HStack>

      {/* MergeView container */}
      <Box
        ref={containerRef}
        flex={1}
        overflow="auto"
        css={{
          "& .cm-mergeView": { height: "100%", overflow: "hidden" },
          "& .cm-mergeViewEditors": { height: "100%", overflow: "hidden" },
          "& .cm-mergeViewEditor": { overflow: "auto" },
          "& .cm-editor": { height: "100%", overflow: "auto" },
          "& .cm-content": { minWidth: 0 },
          "& .cm-block-widget": { maxWidth: "100%", overflow: "hidden" },
          /* Side A (current/deleted) — red background like GitHub */
          "& .cm-mergeViewEditor:first-child .cm-changedLine": {
            backgroundColor: "rgba(248, 81, 73, 0.1) !important",
          },
          "& .cm-mergeViewEditor:first-child .cm-changedText": {
            backgroundColor: "rgba(248, 81, 73, 0.3) !important",
          },
          "& .cm-deletedChunk": {
            backgroundColor: "rgba(248, 81, 73, 0.08) !important",
          },
          /* Side B (proposed/added) — green background like GitHub */
          "& .cm-mergeViewEditor:last-child .cm-changedLine": {
            backgroundColor: "rgba(63, 185, 80, 0.1) !important",
          },
          "& .cm-mergeViewEditor:last-child .cm-changedText": {
            backgroundColor: "rgba(63, 185, 80, 0.3) !important",
          },
          /* Gutter markers */
          "& .cm-changeGutter .cm-gutterElement": {
            color: "rgba(63, 185, 80, 0.7)",
          },
        }}
      />
    </Box>
  );
}
