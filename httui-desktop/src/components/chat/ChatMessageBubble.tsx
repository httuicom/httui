import { memo, useState, useCallback, useRef } from "react";
import { Box, Flex, HStack, IconButton, Image, Text } from "@chakra-ui/react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { LuBot, LuPencil, LuRefreshCw, LuFileDown } from "react-icons/lu";
import { save } from "@tauri-apps/plugin-dialog";
import { writeFile } from "@tauri-apps/plugin-fs";
import type { ChatMessage, ChatToolCall } from "@/lib/tauri/chat";
import type { ToolActivity, ContentSegment } from "@/stores/chat";
import { ChatMarkdown } from "./ChatMarkdown";
import { ToolUseGroup } from "./ToolUseGroup";

interface ContentBlock {
  type: string;
  text?: string;
  path?: string;
  media_type?: string;
  tool_use_ids?: string[];
}

interface ParsedContent {
  segments: ContentSegment[];
  images: { path: string; mediaType: string }[];
  fullText: string;
}

function parseMessageContent(
  contentJson: string,
  toolCalls: ChatToolCall[],
): ParsedContent {
  try {
    const blocks: ContentBlock[] = JSON.parse(contentJson);
    if (Array.isArray(blocks)) {
      const segments: ContentSegment[] = [];
      const images: { path: string; mediaType: string }[] = [];
      const textParts: string[] = [];

      for (const b of blocks) {
        if (b.type === "text" && b.text) {
          segments.push({ type: "text", text: b.text });
          textParts.push(b.text);
        } else if (b.type === "tool_group" && b.tool_use_ids) {
          segments.push({ type: "tool_group", toolUseIds: b.tool_use_ids });
        } else if (b.type === "image" && b.path) {
          images.push({ path: b.path, mediaType: b.media_type ?? "image/png" });
        }
      }

      // Backward compat: old messages have single text block + separate tool_calls
      // If there are tool_calls but no tool_group segments, append one at the end
      if (
        toolCalls.length > 0 &&
        !segments.some((s) => s.type === "tool_group")
      ) {
        segments.push({
          type: "tool_group",
          toolUseIds: toolCalls.map((tc) => tc.tool_use_id),
        });
      }

      return { segments, images, fullText: textParts.join("\n") };
    }
    if (typeof blocks === "string") {
      return {
        segments: [{ type: "text", text: blocks }],
        images: [],
        fullText: blocks,
      };
    }
  } catch {
    return {
      segments: [{ type: "text", text: contentJson }],
      images: [],
      fullText: contentJson,
    };
  }
  return { segments: [], images: [], fullText: "" };
}

function formatTimestamp(unixSeconds: number): string {
  const date = new Date(unixSeconds * 1000);
  const now = new Date();
  const time = date.toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });

  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const msgDay = new Date(date.getFullYear(), date.getMonth(), date.getDate());
  const diffDays = Math.floor((today.getTime() - msgDay.getTime()) / 86400000);

  if (diffDays === 0) return time;
  if (diffDays === 1) return `ontem ${time}`;
  if (diffDays < 7) {
    const dayName = date.toLocaleDateString([], { weekday: "short" });
    return `${dayName} ${time}`;
  }
  return `${date.toLocaleDateString([], { day: "2-digit", month: "2-digit" })} ${time}`;
}

interface ChatMessageBubbleProps {
  message: ChatMessage;
  streamingContent?: string;
  streamingSegments?: ContentSegment[];
  toolActivity?: Map<string, ToolActivity>;
  isLastAssistant?: boolean;
  onEdit?: (turnIndex: number, newText: string) => void;
  onRegenerate?: () => void;
}

export const ChatMessageBubble = memo(function ChatMessageBubble({
  message,
  streamingContent,
  streamingSegments,
  toolActivity,
  isLastAssistant,
  onEdit,
  onRegenerate,
}: ChatMessageBubbleProps) {
  const isUser = message.role === "user";
  const parsed = parseMessageContent(message.content_json, message.tool_calls);
  const content = streamingContent ?? parsed.fullText;

  const [editing, setEditing] = useState(false);
  const [editText, setEditText] = useState("");
  const editRef = useRef<HTMLTextAreaElement>(null);

  const startEdit = useCallback(() => {
    setEditText(parsed.fullText);
    setEditing(true);
    setTimeout(() => editRef.current?.focus(), 0);
  }, [parsed.fullText]);

  const confirmEdit = useCallback(() => {
    if (editText.trim() && onEdit) {
      onEdit(message.turn_index, editText.trim());
    }
    setEditing(false);
  }, [editText, onEdit, message.turn_index]);

  if (isUser) {
    if (editing) {
      return (
        <Box display="flex" justifyContent="flex-end" px={3} py={1.5}>
          <Box maxW="85%" w="100%">
            <textarea
              ref={editRef}
              value={editText}
              onChange={(e) => setEditText(e.target.value)}
              onKeyDown={(e) => {
                e.stopPropagation();
                if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
                  e.preventDefault();
                  confirmEdit();
                }
                if (e.key === "Escape") setEditing(false);
              }}
              onMouseDown={(e) => e.stopPropagation()}
              style={{
                width: "100%",
                background: "var(--chakra-colors-bg-subtle)",
                border: "1px solid var(--chakra-colors-brand-500)",
                borderRadius: "var(--chakra-radii-md)",
                padding: "8px 12px",
                fontSize: "14px",
                fontFamily: "var(--chakra-fonts-body)",
                resize: "none",
                minHeight: "60px",
                outline: "none",
                color: "var(--chakra-colors-fg)",
              }}
            />
            <HStack justify="flex-end" mt={1} gap={1}>
              <Text fontSize="2xs" color="fg.muted">
                Esc cancel · Cmd+Enter send
              </Text>
              <Box
                as="button"
                px={2}
                py={0.5}
                rounded="sm"
                fontSize="xs"
                bg="bg.subtle"
                border="1px solid"
                borderColor="border"
                cursor="pointer"
                onClick={() => setEditing(false)}
              >
                Cancel
              </Box>
              <Box
                as="button"
                px={2}
                py={0.5}
                rounded="sm"
                fontSize="xs"
                bg="brand.500"
                color="white"
                cursor="pointer"
                onClick={confirmEdit}
              >
                Send
              </Box>
            </HStack>
          </Box>
        </Box>
      );
    }

    return (
      <Box
        display="flex"
        justifyContent="flex-end"
        px={3}
        py={1.5}
        role="group"
      >
        {onEdit && message.id > 0 && (
          <IconButton
            aria-label="Edit message"
            size="2xs"
            variant="ghost"
            opacity={0}
            _groupHover={{ opacity: 0.5 }}
            onClick={startEdit}
            alignSelf="center"
            mr={1}
          >
            <LuPencil />
          </IconButton>
        )}
        <Box
          maxW="85%"
          bg="brand.500/10"
          border="1px solid"
          borderColor="brand.500/20"
          rounded="lg"
          roundedBottomRight="sm"
          px={3}
          py={2}
        >
          {parsed.images.length > 0 && (
            <Flex gap={1} mb={content ? 1.5 : 0} flexWrap="wrap">
              {parsed.images.map((img, i) => (
                <Image
                  key={i}
                  src={convertFileSrc(img.path)}
                  alt="attachment"
                  maxH="120px"
                  maxW="200px"
                  rounded="md"
                  objectFit="cover"
                />
              ))}
            </Flex>
          )}
          {content && (
            <Text fontSize="sm" whiteSpace="pre-wrap">
              {content}
            </Text>
          )}
          <Text fontSize="2xs" color="fg.muted" textAlign="right" mt={1}>
            {formatTimestamp(message.created_at)}
          </Text>
        </Box>
      </Box>
    );
  }

  // Assistant message — render segments in order
  const segments =
    streamingSegments && streamingSegments.length > 0
      ? streamingSegments
      : parsed.segments;

  // Build a map of tool_use_id -> ChatToolCall for quick lookup
  const toolCallMap = new Map(
    message.tool_calls.map((tc) => [tc.tool_use_id, tc]),
  );

  return (
    <Box px={3} py={1.5}>
      <HStack align="start" gap={2}>
        <Box
          flexShrink={0}
          w="24px"
          h="24px"
          rounded="full"
          bg="purple.500/20"
          display="flex"
          alignItems="center"
          justifyContent="center"
          mt={0.5}
        >
          <LuBot size={14} />
        </Box>
        <Box flex={1} minW={0}>
          {segments.map((seg, idx) => {
            if (seg.type === "text") {
              return <ChatMarkdown key={idx} content={seg.text} />;
            }
            // tool_group segment
            const groupToolCalls = seg.toolUseIds
              .map((id) => toolCallMap.get(id))
              .filter((tc): tc is ChatToolCall => tc != null);
            // For streaming, filter toolActivity to only this group's IDs
            const groupActivity = toolActivity
              ? new Map(
                  seg.toolUseIds
                    .filter((id) => toolActivity.has(id))
                    .map((id) => [id, toolActivity.get(id)!]),
                )
              : undefined;
            return (
              <ToolUseGroup
                key={idx}
                toolCalls={groupToolCalls}
                toolActivity={groupActivity}
              />
            );
          })}

          {message.is_partial && (
            <Text fontSize="2xs" color="orange.400" mt={1}>
              Response was interrupted
            </Text>
          )}
          <HStack mt={1} gap={1}>
            <Text fontSize="2xs" color="fg.muted">
              {formatTimestamp(message.created_at)}
              {message.tokens_out != null && ` · ${message.tokens_out} tokens`}
            </Text>
            {message.id > 0 && (
              <IconButton
                aria-label="Save as note"
                size="2xs"
                variant="ghost"
                onClick={async () => {
                  const path = await save({
                    filters: [{ name: "Markdown", extensions: ["md"] }],
                    defaultPath: "chat-response.md",
                  });
                  if (path) {
                    await writeFile(path, new TextEncoder().encode(content));
                  }
                }}
              >
                <LuFileDown size={10} />
              </IconButton>
            )}
            {isLastAssistant && onRegenerate && message.id > 0 && (
              <IconButton
                aria-label="Regenerate"
                size="2xs"
                variant="ghost"
                onClick={onRegenerate}
              >
                <LuRefreshCw size={10} />
              </IconButton>
            )}
          </HStack>
        </Box>
      </HStack>
    </Box>
  );
});
