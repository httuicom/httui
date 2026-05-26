import { invoke } from "@tauri-apps/api/core";

// --- Types ---

export interface ChatSession {
  id: number;
  claude_session_id: string | null;
  title: string;
  cwd: string | null;
  created_at: number;
  updated_at: number;
  archived_at: number | null;
}

export interface ChatToolCall {
  id: number;
  tool_use_id: string;
  tool_name: string;
  input_json: string;
  result_json: string | null;
  is_error: boolean;
  created_at: number;
}

export interface ChatMessage {
  id: number;
  session_id: number;
  role: "user" | "assistant";
  turn_index: number;
  content_json: string;
  tokens_in: number | null;
  tokens_out: number | null;
  is_partial: boolean;
  created_at: number;
  tool_calls: ChatToolCall[];
}

export interface AttachmentInput {
  media_type: string;
  path: string;
}

// --- Commands ---

export function createChatSession(cwd?: string): Promise<ChatSession> {
  return invoke("create_chat_session", { cwd: cwd ?? null });
}

export function listChatSessions(): Promise<ChatSession[]> {
  return invoke("list_chat_sessions");
}

export function getChatSession(sessionId: number): Promise<ChatSession> {
  return invoke("get_chat_session", { sessionId });
}

export function archiveChatSession(sessionId: number): Promise<void> {
  return invoke("archive_chat_session", { sessionId });
}

export function listChatMessages(sessionId: number): Promise<ChatMessage[]> {
  return invoke("list_chat_messages", { sessionId });
}

export function sendChatMessage(
  sessionId: number,
  text: string,
  attachments: AttachmentInput[] = [],
): Promise<string> {
  return invoke("send_chat_message", { sessionId, text, attachments });
}

export function abortChat(requestId: string): Promise<void> {
  return invoke("abort_chat", { requestId });
}

export function saveAttachmentTmp(
  bytes: number[],
  mediaType: string,
): Promise<string> {
  return invoke("save_attachment_tmp", { bytes, mediaType });
}

export function respondChatPermission(
  permissionId: string,
  behavior: "allow" | "deny",
  scope: "once" | "session" | "always" = "once",
  toolName?: string,
  message?: string,
): Promise<void> {
  return invoke("respond_chat_permission", {
    permissionId,
    behavior,
    scope,
    toolName: toolName ?? null,
    message: message ?? null,
  });
}

export function deleteMessagesAfter(
  sessionId: number,
  turnIndex: number,
): Promise<void> {
  return invoke("delete_messages_after", { sessionId, turnIndex });
}

export function clearSessionClaudeId(sessionId: number): Promise<void> {
  return invoke("clear_session_claude_id", { sessionId });
}

export function updateChatSessionCwd(
  sessionId: number,
  cwd: string | null,
): Promise<void> {
  return invoke("update_chat_session_cwd", { sessionId, cwd });
}

// --- Permission management ---

export interface ToolPermission {
  id: number;
  tool_name: string;
  path_pattern: string | null;
  workspace: string | null;
  scope: string;
  behavior: string;
  session_id: number | null;
  created_at: number;
}

export function listToolPermissions(
  workspace?: string,
): Promise<ToolPermission[]> {
  return invoke("list_tool_permissions", { workspace: workspace ?? null });
}

export function deleteToolPermission(id: number): Promise<void> {
  return invoke("delete_tool_permission", { id });
}

// --- Usage stats ---

export interface DailyUsage {
  date: string;
  input_tokens: number;
  output_tokens: number;
  cache_read_tokens: number;
}

export function getUsageStats(from: string, to: string): Promise<DailyUsage[]> {
  return invoke("get_usage_stats", { from, to });
}
