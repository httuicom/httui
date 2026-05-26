import { create } from "zustand";
import { devtools } from "zustand/middleware";
import { listen } from "@tauri-apps/api/event";
import {
  listChatMessages,
  listChatSessions,
  createChatSession,
  archiveChatSession,
  sendChatMessage,
  abortChat,
  respondChatPermission,
  deleteMessagesAfter,
  clearSessionClaudeId,
  updateChatSessionCwd,
  type ChatSession,
  type ChatMessage,
  type AttachmentInput,
} from "@/lib/tauri/chat";
import { usePaneStore } from "@/stores/pane";
import { forceReloadFile } from "@/lib/tauri/commands";

// --- Types ---

interface ChatDeltaPayload {
  session_id: number;
  text: string;
}

interface ChatDonePayload {
  session_id: number;
  usage: {
    input_tokens: number;
    output_tokens: number;
    cache_read_tokens: number;
  } | null;
  stop_reason: string | null;
}

interface ChatErrorPayload {
  session_id: number;
  category: string;
  message: string;
}

interface ChatToolUsePayload {
  session_id: number;
  tool_use_id: string;
  name: string;
  input: Record<string, unknown>;
}

interface ChatToolResultPayload {
  session_id: number;
  tool_use_id: string;
  content: unknown[];
  is_error: boolean;
}

interface ChatPermissionRequestPayload {
  session_id: number;
  permission_id: string;
  tool_name: string;
  tool_input: Record<string, unknown>;
}

export interface ToolActivity {
  name: string;
  input: Record<string, unknown>;
  result?: string;
  isError?: boolean;
  pending: boolean;
}

export type ContentSegment =
  | { type: "text"; text: string }
  | { type: "tool_group"; toolUseIds: string[] };

export interface PendingPermission {
  permissionId: string;
  toolName: string;
  toolInput: Record<string, unknown>;
}

interface PendingFileUpdate {
  filePath: string;
  toolUseId: string;
}

interface ChatState {
  // Sessions
  sessions: ChatSession[];
  activeSessionId: number | null;
  activeSession: ChatSession | null;

  // Messages
  messages: ChatMessage[];
  streamingContent: string;
  streamingSegments: ContentSegment[];
  isStreaming: boolean;
  error: string | null;

  // Tools
  toolActivity: Map<string, ToolActivity>;
  pendingPermission: PendingPermission | null;
  resumeFailed: boolean;

  // Session actions
  initSessions: () => Promise<void>;
  refreshSessions: () => Promise<void>;
  selectSession: (id: number) => void;
  createSession: () => Promise<ChatSession>;
  archiveSession: (id: number) => Promise<void>;
  updateCwd: (cwd: string | null) => Promise<void>;

  // Chat actions
  sendMessage: (text: string, attachments?: AttachmentInput[]) => Promise<void>;
  abort: () => void;
  respondPermission: (
    permissionId: string,
    behavior: "allow" | "deny",
    scope?: "once" | "session" | "always",
  ) => Promise<void>;
  editAndResend: (turnIndex: number, newText: string) => Promise<void>;
  regenerate: () => Promise<void>;
  resetAndContinue: () => Promise<void>;
}

// --- Module-level mutable state (not reactive, no re-render needed) ---
let contentAccumulator = "";
let segmentsAccumulator: ContentSegment[] = [];
let rafId = 0;
let activeRequestId: string | null = null;
const pendingFileUpdates: PendingFileUpdate[] = [];

// Helper to get vault path for file reload
function getVaultPathForFile(filePath: string): string | null {
  // Look through open tabs to find the vault path for a file
  const { layout } = usePaneStore.getState();
  function searchLayout(
    node: import("@/types/pane").PaneLayout,
  ): string | null {
    if (node.type === "leaf") {
      const tab = node.tabs.find((t) => t.filePath === filePath);
      return tab?.vaultPath ?? null;
    }
    return searchLayout(node.children[0]) ?? searchLayout(node.children[1]);
  }
  return searchLayout(layout);
}

function resetStreamingState() {
  contentAccumulator = "";
  segmentsAccumulator = [];
  activeRequestId = null;
  cancelAnimationFrame(rafId);
}

// --- Store ---

export const useChatStore = create<ChatState>()(
  devtools(
    (set, get) => ({
      // Initial state
      sessions: [],
      activeSessionId: null,
      activeSession: null,
      messages: [],
      streamingContent: "",
      streamingSegments: [],
      isStreaming: false,
      error: null,
      toolActivity: new Map(),
      pendingPermission: null,
      resumeFailed: false,

      // --- Session actions ---

      initSessions: async () => {
        try {
          const list = await listChatSessions();
          if (list.length > 0 && list[0].title === "Nova conversa") {
            set({
              sessions: list,
              activeSessionId: list[0].id,
              activeSession: list[0],
            });
          } else {
            const session = await createChatSession();
            const refreshed = await listChatSessions();
            set({
              sessions: refreshed,
              activeSessionId: session.id,
              activeSession: refreshed.find((s) => s.id === session.id) ?? null,
            });
          }
        } catch (e) {
          console.error("Failed to init chat sessions:", e);
        }
      },

      refreshSessions: async () => {
        try {
          const list = await listChatSessions();
          const { activeSessionId } = get();
          set({
            sessions: list,
            activeSession: list.find((s) => s.id === activeSessionId) ?? null,
          });
        } catch (e) {
          console.error("Failed to list chat sessions:", e);
        }
      },

      selectSession: (id) => {
        const { sessions } = get();
        set({
          activeSessionId: id,
          activeSession: sessions.find((s) => s.id === id) ?? null,
        });
      },

      createSession: async () => {
        const session = await createChatSession();
        const list = await listChatSessions();
        set({
          sessions: list,
          activeSessionId: session.id,
          activeSession: list.find((s) => s.id === session.id) ?? null,
        });
        return session;
      },

      archiveSession: async (id) => {
        try {
          await archiveChatSession(id);
          const remaining = await listChatSessions();
          if (remaining.length === 0) {
            const session = await createChatSession();
            set({
              sessions: [session],
              activeSessionId: session.id,
              activeSession: session,
            });
          } else {
            const { activeSessionId } = get();
            const newActiveId =
              activeSessionId === id ? remaining[0].id : activeSessionId;
            set({
              sessions: remaining,
              activeSessionId: newActiveId,
              activeSession:
                remaining.find((s) => s.id === newActiveId) ?? null,
            });
          }
        } catch (e) {
          console.error("Failed to archive chat session:", e);
        }
      },

      updateCwd: async (cwd) => {
        const { activeSessionId, refreshSessions } = get();
        if (activeSessionId === null) return;
        await updateChatSessionCwd(activeSessionId, cwd);
        await refreshSessions();
      },

      // --- Chat actions ---

      sendMessage: async (text, attachments) => {
        const { activeSessionId, messages } = get();
        if (activeSessionId === null) return;
        const hasContent =
          text.trim() || (attachments && attachments.length > 0);
        if (!hasContent) return;

        const optimisticMsg: ChatMessage = {
          id: -Date.now(),
          session_id: activeSessionId,
          role: "user",
          turn_index: messages.length,
          content_json: JSON.stringify([{ type: "text", text }]),
          tokens_in: null,
          tokens_out: null,
          is_partial: false,
          created_at: Math.floor(Date.now() / 1000),
          tool_calls: [],
        };

        resetStreamingState();
        set({
          error: null,
          toolActivity: new Map(),
          messages: [...messages, optimisticMsg],
          streamingContent: "",
          streamingSegments: [],
          isStreaming: true,
        });

        try {
          const requestId = await sendChatMessage(
            activeSessionId,
            text,
            attachments ?? [],
          );
          activeRequestId = requestId;
        } catch (e) {
          set({
            isStreaming: false,
            error: e instanceof Error ? e.message : String(e),
          });
        }
      },

      abort: () => {
        if (activeRequestId) {
          abortChat(activeRequestId).catch(console.error);
        }
      },

      respondPermission: async (permissionId, behavior, scope = "once") => {
        try {
          const toolName = get().pendingPermission?.toolName;
          await respondChatPermission(permissionId, behavior, scope, toolName);
          set({ pendingPermission: null });
        } catch (e) {
          console.error("Failed to respond to permission:", e);
        }
      },

      editAndResend: async (turnIndex, newText) => {
        const { activeSessionId, sendMessage } = get();
        if (activeSessionId === null) return;
        await deleteMessagesAfter(activeSessionId, turnIndex);
        const msgs = await listChatMessages(activeSessionId);
        set({ messages: msgs });
        await sendMessage(newText);
      },

      regenerate: async () => {
        const { activeSessionId, messages, sendMessage } = get();
        if (activeSessionId === null || messages.length < 2) return;
        const lastUserMsg = [...messages]
          .reverse()
          .find((m) => m.role === "user");
        if (!lastUserMsg) return;
        await deleteMessagesAfter(activeSessionId, lastUserMsg.turn_index + 1);
        const msgs = await listChatMessages(activeSessionId);
        set({ messages: msgs });
        try {
          const blocks = JSON.parse(lastUserMsg.content_json);
          const text = Array.isArray(blocks)
            ? blocks
                .filter((b: { type: string }) => b.type === "text")
                .map((b: { text: string }) => b.text)
                .join("\n")
            : String(blocks);
          await sendMessage(text);
        } catch {
          await sendMessage(lastUserMsg.content_json);
        }
      },

      resetAndContinue: async () => {
        const { activeSessionId, messages, sendMessage } = get();
        if (activeSessionId === null) return;
        await clearSessionClaudeId(activeSessionId);
        set({ resumeFailed: false, error: null });
        const lastUserMsg = [...messages]
          .reverse()
          .find((m) => m.role === "user");
        if (lastUserMsg) {
          try {
            const blocks = JSON.parse(lastUserMsg.content_json);
            const text = Array.isArray(blocks)
              ? blocks
                  .filter((b: { type: string }) => b.type === "text")
                  .map((b: { text: string }) => b.text)
                  .join("\n")
              : String(blocks);
            await sendMessage(text);
          } catch {
            /* fallback */
          }
        }
      },
    }),
    { name: "chat-store" },
  ),
);

// --- Tauri event listeners ---

export function setupChatListeners() {
  listen<ChatDeltaPayload>("chat:delta", (event) => {
    const { activeSessionId } = useChatStore.getState();
    if (event.payload.session_id !== activeSessionId) return;

    contentAccumulator += event.payload.text;
    const segs = segmentsAccumulator;
    const last = segs[segs.length - 1];
    if (last && last.type === "text") {
      last.text += event.payload.text;
    } else {
      segs.push({ type: "text", text: event.payload.text });
    }
    cancelAnimationFrame(rafId);
    rafId = requestAnimationFrame(() => {
      useChatStore.setState({
        streamingContent: contentAccumulator,
        streamingSegments: [...segs],
      });
    });
  });

  listen<ChatToolUsePayload>("chat:tool_use", (event) => {
    const { activeSessionId } = useChatStore.getState();
    if (event.payload.session_id !== activeSessionId) return;

    const { tool_use_id, name, input } = event.payload;
    const segs = segmentsAccumulator;
    const last = segs[segs.length - 1];
    if (last && last.type === "tool_group") {
      last.toolUseIds.push(tool_use_id);
    } else {
      segs.push({ type: "tool_group", toolUseIds: [tool_use_id] });
    }

    const { toolActivity } = useChatStore.getState();
    const next = new Map(toolActivity);
    next.set(tool_use_id, { name, input, pending: true });

    useChatStore.setState({
      streamingSegments: [...segs],
      toolActivity: next,
    });

    // Track update_note calls for auto-save suppression
    if (name.includes("update_note") && input.path) {
      const filePath = String(input.path);
      pendingFileUpdates.push({ filePath, toolUseId: tool_use_id });
      // Suppress auto-save via pane store
      // useEditorSession suppressAutoSave is not accessible here,
      // but we can use the same pattern via forceReloadFile on complete
    }
  });

  listen<ChatToolResultPayload>("chat:tool_result", (event) => {
    const { activeSessionId } = useChatStore.getState();
    if (event.payload.session_id !== activeSessionId) return;

    const { tool_use_id, content, is_error } = event.payload;
    const resultText = content
      .map((c: unknown) => {
        if (typeof c === "object" && c !== null && "text" in c) {
          return (c as { text: string }).text;
        }
        return JSON.stringify(c);
      })
      .join("\n");

    const { toolActivity } = useChatStore.getState();
    const next = new Map(toolActivity);
    const existing = next.get(tool_use_id);
    if (existing) {
      next.set(tool_use_id, {
        ...existing,
        result: resultText,
        isError: is_error,
        pending: false,
      });
    }
    useChatStore.setState({ toolActivity: next });

    // Check if this is a completed update_note — force reload
    const idx = pendingFileUpdates.findIndex(
      (p) => p.toolUseId === tool_use_id,
    );
    if (idx >= 0) {
      const { filePath } = pendingFileUpdates[idx];
      pendingFileUpdates.splice(idx, 1);
      if (!is_error) {
        const vaultPath = getVaultPathForFile(filePath);
        if (vaultPath) {
          forceReloadFile(vaultPath, filePath).catch(() => {});
        }
      }
    }
  });

  listen<ChatPermissionRequestPayload>("chat:permission_request", (event) => {
    const { activeSessionId } = useChatStore.getState();
    if (event.payload.session_id !== activeSessionId) return;

    const { permission_id, tool_name, tool_input } = event.payload;
    useChatStore.setState({
      pendingPermission: {
        permissionId: permission_id,
        toolName: tool_name,
        toolInput: tool_input,
      },
    });
  });

  listen<ChatDonePayload>("chat:done", async (event) => {
    const { activeSessionId } = useChatStore.getState();
    if (event.payload.session_id !== activeSessionId) return;

    resetStreamingState();
    useChatStore.setState({
      streamingContent: "",
      streamingSegments: [],
      isStreaming: false,
      toolActivity: new Map(),
      pendingPermission: null,
    });

    try {
      const msgs = await listChatMessages(activeSessionId);
      useChatStore.setState({ messages: msgs });
    } catch (e) {
      console.error("Failed to refresh messages:", e);
    }
  });

  listen<ChatErrorPayload>("chat:error", async (event) => {
    const { activeSessionId } = useChatStore.getState();
    if (event.payload.session_id !== activeSessionId) return;

    resetStreamingState();

    const { category, message } = event.payload;
    let errorMsg: string;
    let resumeFailed = false;
    if (category === "auth") {
      errorMsg =
        "Authentication required. Run `claude login` in your terminal.";
    } else if (category === "rate_limit") {
      errorMsg = "Rate limit reached. Please wait a moment.";
    } else if (category === "resume_failed") {
      resumeFailed = true;
      errorMsg = "Session could not be resumed.";
    } else {
      errorMsg = message;
    }

    useChatStore.setState({
      streamingContent: "",
      streamingSegments: [],
      isStreaming: false,
      toolActivity: new Map(),
      pendingPermission: null,
      error: errorMsg,
      resumeFailed,
    });

    try {
      const msgs = await listChatMessages(activeSessionId);
      useChatStore.setState({ messages: msgs });
    } catch (e) {
      console.error("Failed to refresh messages:", e);
    }
  });

  // Session title updates
  listen("chat:session-updated", () => {
    useChatStore.getState().refreshSessions();
  });
}

// --- Session change subscription ---
// When activeSessionId changes, load messages for the new session
let prevSessionId: number | null = null;

export function setupSessionWatcher() {
  useChatStore.subscribe((state) => {
    if (state.activeSessionId !== prevSessionId) {
      prevSessionId = state.activeSessionId;

      if (state.activeSessionId === null) {
        useChatStore.setState({
          messages: [],
          streamingContent: "",
          streamingSegments: [],
          isStreaming: false,
          error: null,
          toolActivity: new Map(),
          pendingPermission: null,
        });
      } else {
        listChatMessages(state.activeSessionId)
          .then((msgs) => useChatStore.setState({ messages: msgs }))
          .catch(console.error);
      }
    }
  });
}
