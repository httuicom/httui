import { memo, useState } from "react";
import { Box, HStack, Text } from "@chakra-ui/react";
import {
  LuChevronDown,
  LuChevronRight,
  LuLoader,
  LuCheck,
  LuX,
  LuFileText,
  LuSearch,
  LuTerminal,
  LuPencil,
  LuFolderSearch,
  LuGlobe,
  LuWrench,
} from "react-icons/lu";
import type { ChatToolCall } from "@/lib/tauri/chat";
import type { ToolActivity } from "@/stores/chat";

type ToolItem =
  | { kind: "persisted"; data: ChatToolCall }
  | { kind: "live"; id: string; data: ToolActivity };

interface ToolUseGroupProps {
  toolCalls: ChatToolCall[];
  toolActivity?: Map<string, ToolActivity>;
}

function shortName(name: string): string {
  const parts = name.split("__");
  return parts.length > 2 ? parts.slice(2).join("__") : name;
}

function toolIcon(name: string) {
  const n = name.toLowerCase();
  if (n.includes("read") || n.includes("cat")) return LuFileText;
  if (
    n.includes("write") ||
    n.includes("edit") ||
    n.includes("create") ||
    n.includes("update") ||
    n.includes("delete")
  )
    return LuPencil;
  if (n.includes("grep") || n.includes("search")) return LuSearch;
  if (n.includes("glob") || n.includes("list")) return LuFolderSearch;
  if (n.includes("bash") || n.includes("exec")) return LuTerminal;
  if (n.includes("fetch") || n.includes("web")) return LuGlobe;
  return LuWrench;
}

function isWriteTool(name: string): boolean {
  const n = name.toLowerCase();
  return (
    n.includes("write") ||
    n.includes("update") ||
    n.includes("create") ||
    n.includes("edit") ||
    n.includes("delete")
  );
}

function inlineSummary(input: unknown): string | null {
  if (!input || typeof input !== "object") return null;
  const obj = input as Record<string, unknown>;
  if ("command" in obj) return String(obj.command);
  if ("file_path" in obj) return String(obj.file_path);
  if ("path" in obj && "pattern" in obj) return `${obj.pattern} in ${obj.path}`;
  if ("pattern" in obj) return String(obj.pattern);
  if ("path" in obj) return String(obj.path);
  if ("note_path" in obj) return String(obj.note_path);
  return null;
}

function parseDiffStats(
  result: string | null | undefined,
): { added: number; removed: number } | null {
  if (!result) return null;
  try {
    const parsed = JSON.parse(result);
    if (parsed && typeof parsed.lines_added === "number") {
      return { added: parsed.lines_added, removed: parsed.lines_removed ?? 0 };
    }
  } catch {
    /* not JSON */
  }
  return null;
}

function getItemInfo(item: ToolItem) {
  if (item.kind === "persisted") {
    const tc = item.data;
    const input =
      typeof tc.input_json === "string"
        ? (() => {
            try {
              return JSON.parse(tc.input_json);
            } catch {
              return tc.input_json;
            }
          })()
        : tc.input_json;
    return {
      rawName: tc.tool_name,
      name: shortName(tc.tool_name),
      input,
      result: tc.result_json,
      isError: tc.is_error,
      isPending: false,
    };
  }
  const act = item.data;
  return {
    rawName: act.name,
    name: shortName(act.name),
    input: act.input,
    result: act.result,
    isError: act.isError ?? false,
    isPending: act.pending,
  };
}

function formatInput(input: unknown): string {
  if (typeof input === "string") {
    try {
      return JSON.stringify(JSON.parse(input), null, 2);
    } catch {
      return input;
    }
  }
  return JSON.stringify(input, null, 2);
}

export const ToolUseGroup = memo(function ToolUseGroup({
  toolCalls,
  toolActivity,
}: ToolUseGroupProps) {
  const [expanded, setExpanded] = useState(false);
  const [expandedItems, setExpandedItems] = useState<Set<string>>(new Set());

  // Build unified list
  const items: ToolItem[] = [];
  for (const tc of toolCalls) {
    items.push({ kind: "persisted", data: tc });
  }
  if (toolActivity) {
    for (const [id, act] of toolActivity.entries()) {
      items.push({ kind: "live", id, data: act });
    }
  }

  if (items.length === 0) return null;

  const pendingCount = items.filter((it) => {
    const info = getItemInfo(it);
    return info.isPending;
  }).length;

  const errorCount = items.filter((it) => {
    const info = getItemInfo(it);
    return info.isError;
  }).length;

  const toggleItem = (key: string) => {
    setExpandedItems((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  const itemKey = (item: ToolItem, _idx: number) =>
    item.kind === "persisted" ? item.data.tool_use_id : item.id;

  // Summary line
  const summaryParts: string[] = [];
  if (pendingCount > 0) summaryParts.push(`${pendingCount} running`);
  if (errorCount > 0) summaryParts.push(`${errorCount} failed`);
  const summaryText =
    pendingCount > 0
      ? `${items.length} tools · ${summaryParts.join(", ")}`
      : `${items.length} tool${items.length !== 1 ? "s" : ""} used${errorCount > 0 ? ` · ${errorCount} failed` : ""}`;

  return (
    <Box
      my={1}
      rounded="md"
      border="1px solid"
      borderColor="border"
      overflow="hidden"
    >
      {/* Group header */}
      <HStack
        px={2}
        py={1.5}
        cursor="pointer"
        onClick={() => setExpanded((prev) => !prev)}
        gap={1.5}
        _hover={{ bg: "bg.subtle" }}
        userSelect="none"
      >
        <Box color="fg.muted" flexShrink={0}>
          {expanded ? (
            <LuChevronDown size={12} />
          ) : (
            <LuChevronRight size={12} />
          )}
        </Box>
        {pendingCount > 0 ? (
          <Box color="blue.400" flexShrink={0}>
            <LuLoader size={12} className="animate-spin" />
          </Box>
        ) : errorCount > 0 ? (
          <Box color="red.400" flexShrink={0}>
            <LuX size={12} />
          </Box>
        ) : (
          <Box color="green.400" flexShrink={0}>
            <LuCheck size={12} />
          </Box>
        )}
        <Text fontSize="2xs" fontWeight="medium" color="fg.muted">
          {summaryText}
        </Text>
      </HStack>

      {/* Expanded: list each tool */}
      {expanded && (
        <Box borderTop="1px solid" borderColor="border">
          {items.map((item, idx) => {
            const key = itemKey(item, idx);
            const info = getItemInfo(item);
            const Icon = toolIcon(info.rawName);
            const summary = inlineSummary(info.input);
            const isWrite = isWriteTool(info.rawName);
            const diffStats = isWrite ? parseDiffStats(info.result) : null;
            const isItemExpanded = expandedItems.has(key);

            const statusColor = info.isPending
              ? "blue.400"
              : info.isError
                ? "red.400"
                : "green.400";
            const StatusIcon = info.isPending
              ? LuLoader
              : info.isError
                ? LuX
                : LuCheck;

            return (
              <Box key={key}>
                <HStack
                  px={2}
                  py={1}
                  gap={1.5}
                  cursor="pointer"
                  onClick={() => toggleItem(key)}
                  _hover={{ bg: "bg.subtle" }}
                  borderTop={idx > 0 ? "1px solid" : undefined}
                  borderColor="border"
                >
                  <Box color={statusColor} flexShrink={0}>
                    <StatusIcon
                      size={11}
                      className={info.isPending ? "animate-spin" : undefined}
                    />
                  </Box>
                  <Box color="fg.muted" flexShrink={0}>
                    <Icon size={11} />
                  </Box>
                  <Text
                    fontWeight="medium"
                    fontSize="2xs"
                    flexShrink={0}
                    fontFamily="mono"
                  >
                    {info.name}
                  </Text>
                  {summary && (
                    <Text
                      fontSize="2xs"
                      color="fg.muted"
                      truncate
                      flex={1}
                      fontFamily="mono"
                    >
                      {summary}
                    </Text>
                  )}
                  {isWrite && diffStats && (
                    <HStack gap={1} flexShrink={0}>
                      {diffStats.added > 0 && (
                        <Text
                          fontSize="2xs"
                          color="green.400"
                          fontFamily="mono"
                        >
                          +{diffStats.added}
                        </Text>
                      )}
                      {diffStats.removed > 0 && (
                        <Text fontSize="2xs" color="red.400" fontFamily="mono">
                          -{diffStats.removed}
                        </Text>
                      )}
                    </HStack>
                  )}
                  <Box color="fg.muted" flexShrink={0} ml="auto">
                    {isItemExpanded ? (
                      <LuChevronDown size={10} />
                    ) : (
                      <LuChevronRight size={10} />
                    )}
                  </Box>
                </HStack>

                {/* Item detail */}
                {isItemExpanded && (
                  <Box
                    px={3}
                    py={1.5}
                    bg="bg.subtle/50"
                    borderTop="1px solid"
                    borderColor="border"
                  >
                    <Text
                      fontSize="2xs"
                      color="fg.muted"
                      fontWeight="semibold"
                      mb={0.5}
                    >
                      Input
                    </Text>
                    <Box
                      as="pre"
                      bg="bg.subtle"
                      rounded="sm"
                      px={2}
                      py={1}
                      fontSize="2xs"
                      fontFamily="mono"
                      whiteSpace="pre-wrap"
                      wordBreak="break-all"
                      maxH="120px"
                      overflowY="auto"
                      mb={info.result ? 2 : 0}
                    >
                      {formatInput(info.input)}
                    </Box>
                    {info.result && (
                      <>
                        <Text
                          fontSize="2xs"
                          color="fg.muted"
                          fontWeight="semibold"
                          mb={0.5}
                        >
                          Result
                        </Text>
                        <Box
                          as="pre"
                          bg={info.isError ? "red.500/5" : "bg.subtle"}
                          border={info.isError ? "1px solid" : undefined}
                          borderColor={info.isError ? "red.500/20" : undefined}
                          rounded="sm"
                          px={2}
                          py={1}
                          fontSize="2xs"
                          fontFamily="mono"
                          whiteSpace="pre-wrap"
                          wordBreak="break-all"
                          maxH="200px"
                          overflowY="auto"
                          color={info.isError ? "red.400" : undefined}
                        >
                          {info.result.length > 2000
                            ? info.result.slice(0, 2000) + "\n... (truncated)"
                            : info.result}
                        </Box>
                      </>
                    )}
                    {info.isPending && (
                      <Text fontSize="2xs" color="blue.400" mt={1}>
                        Executing...
                      </Text>
                    )}
                  </Box>
                )}
              </Box>
            );
          })}
        </Box>
      )}
    </Box>
  );
});
