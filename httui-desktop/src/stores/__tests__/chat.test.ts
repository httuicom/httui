import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import {
  useChatStore,
  setupChatListeners,
  setupSessionWatcher,
} from "@/stores/chat";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";
import {
  emitTauriEvent,
  clearTauriListeners,
  listen,
} from "@/test/mocks/tauri-event";
import type { ChatSession, ChatMessage } from "@/lib/tauri/chat";

const mkSession = (
  id: number,
  title = "Nova conversa",
  over: Partial<ChatSession> = {},
): ChatSession => ({
  id,
  claude_session_id: null,
  title,
  cwd: null,
  created_at: 1000,
  updated_at: 1000,
  archived_at: null,
  ...over,
});

const mkMessage = (
  id: number,
  sessionId: number,
  role: "user" | "assistant",
  text: string,
  turn = 0,
): ChatMessage => ({
  id,
  session_id: sessionId,
  role,
  turn_index: turn,
  content_json: JSON.stringify([{ type: "text", text }]),
  tokens_in: null,
  tokens_out: null,
  is_partial: false,
  created_at: 1000,
  tool_calls: [],
});

function resetStore() {
  useChatStore.setState({
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
  });
}

describe("chatStore", () => {
  beforeEach(() => {
    resetStore();
    clearTauriMocks();
    clearTauriListeners();
    listen.mockClear();
    // Make rAF synchronous for predictable streaming assertions
    vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => {
      cb(0);
      return 0;
    });
    vi.stubGlobal("cancelAnimationFrame", () => {});
  });

  afterEach(() => {
    clearTauriMocks();
    clearTauriListeners();
    vi.unstubAllGlobals();
  });

  // ──────────────────────────────────────────────
  // Sessions
  // ──────────────────────────────────────────────
  describe("initSessions", () => {
    it("activates first session if it's a fresh 'Nova conversa'", async () => {
      const sessions = [
        mkSession(1, "Nova conversa"),
        mkSession(2, "Old talk"),
      ];
      mockTauriCommand("list_chat_sessions", () => sessions);

      await useChatStore.getState().initSessions();

      expect(useChatStore.getState().activeSessionId).toBe(1);
      expect(useChatStore.getState().activeSession?.title).toBe(
        "Nova conversa",
      );
    });

    it("creates a new session when first is not 'Nova conversa'", async () => {
      const created = mkSession(99, "Nova conversa");
      let createCalls = 0;
      mockTauriCommand("list_chat_sessions", () => {
        if (createCalls === 0) return [mkSession(1, "Old talk")];
        return [created, mkSession(1, "Old talk")];
      });
      mockTauriCommand("create_chat_session", () => {
        createCalls++;
        return created;
      });

      await useChatStore.getState().initSessions();

      expect(createCalls).toBe(1);
      expect(useChatStore.getState().activeSessionId).toBe(99);
    });

    it("creates session when list is empty", async () => {
      const created = mkSession(7);
      let calls = 0;
      mockTauriCommand("list_chat_sessions", () => {
        if (calls === 0) {
          calls++;
          return [];
        }
        return [created];
      });
      mockTauriCommand("create_chat_session", () => created);

      await useChatStore.getState().initSessions();
      expect(useChatStore.getState().activeSessionId).toBe(7);
    });

    it("logs error and does not throw", async () => {
      mockTauriCommand("list_chat_sessions", () => {
        throw new Error("db down");
      });
      const errSpy = vi.spyOn(console, "error").mockImplementation(() => {});

      await expect(
        useChatStore.getState().initSessions(),
      ).resolves.toBeUndefined();
      expect(errSpy).toHaveBeenCalled();
      errSpy.mockRestore();
    });
  });

  describe("refreshSessions", () => {
    it("updates list and reselects activeSession by id", async () => {
      useChatStore.setState({ activeSessionId: 1 });
      const updated = [mkSession(1, "renamed"), mkSession(2, "x")];
      mockTauriCommand("list_chat_sessions", () => updated);

      await useChatStore.getState().refreshSessions();

      expect(useChatStore.getState().activeSession?.title).toBe("renamed");
    });

    it("logs error when listing fails", async () => {
      mockTauriCommand("list_chat_sessions", () => {
        throw new Error("oops");
      });
      const errSpy = vi.spyOn(console, "error").mockImplementation(() => {});

      await useChatStore.getState().refreshSessions();
      expect(errSpy).toHaveBeenCalled();
      errSpy.mockRestore();
    });
  });

  describe("selectSession", () => {
    it("updates activeSession from sessions list", () => {
      const a = mkSession(1, "A");
      const b = mkSession(2, "B");
      useChatStore.setState({ sessions: [a, b], activeSessionId: 1 });

      useChatStore.getState().selectSession(2);

      expect(useChatStore.getState().activeSessionId).toBe(2);
      expect(useChatStore.getState().activeSession?.title).toBe("B");
    });

    it("sets activeSession to null when id not found", () => {
      useChatStore.setState({ sessions: [mkSession(1)] });
      useChatStore.getState().selectSession(99);
      expect(useChatStore.getState().activeSession).toBeNull();
    });
  });

  describe("createSession", () => {
    it("creates and activates", async () => {
      const session = mkSession(42);
      mockTauriCommand("create_chat_session", () => session);
      mockTauriCommand("list_chat_sessions", () => [session]);

      const result = await useChatStore.getState().createSession();

      expect(result).toEqual(session);
      expect(useChatStore.getState().activeSessionId).toBe(42);
    });
  });

  describe("archiveSession", () => {
    it("creates a new fresh session when last one is archived", async () => {
      const created = mkSession(99);
      let archiveCalls = 0;
      let listCalls = 0;
      mockTauriCommand("archive_chat_session", () => {
        archiveCalls++;
      });
      mockTauriCommand("list_chat_sessions", () => {
        listCalls++;
        return listCalls === 1 ? [] : [created];
      });
      mockTauriCommand("create_chat_session", () => created);

      useChatStore.setState({
        sessions: [mkSession(1)],
        activeSessionId: 1,
      });

      await useChatStore.getState().archiveSession(1);

      expect(archiveCalls).toBe(1);
      expect(useChatStore.getState().activeSessionId).toBe(99);
    });

    it("switches to first remaining session when archiving the active one", async () => {
      mockTauriCommand("archive_chat_session", () => {});
      mockTauriCommand("list_chat_sessions", () => [mkSession(2)]);

      useChatStore.setState({
        sessions: [mkSession(1), mkSession(2)],
        activeSessionId: 1,
      });

      await useChatStore.getState().archiveSession(1);

      expect(useChatStore.getState().activeSessionId).toBe(2);
    });

    it("keeps active when archiving a different session", async () => {
      mockTauriCommand("archive_chat_session", () => {});
      mockTauriCommand("list_chat_sessions", () => [mkSession(1)]);

      useChatStore.setState({
        sessions: [mkSession(1), mkSession(2)],
        activeSessionId: 1,
      });

      await useChatStore.getState().archiveSession(2);

      expect(useChatStore.getState().activeSessionId).toBe(1);
    });
  });

  describe("updateCwd", () => {
    it("calls IPC and refreshes when active session exists", async () => {
      let received: unknown = null;
      mockTauriCommand("update_chat_session_cwd", (args) => {
        received = args;
      });
      mockTauriCommand("list_chat_sessions", () => [
        mkSession(1, "A", { cwd: "/new" }),
      ]);

      useChatStore.setState({
        sessions: [mkSession(1)],
        activeSessionId: 1,
      });

      await useChatStore.getState().updateCwd("/new");

      expect(received).toEqual({ sessionId: 1, cwd: "/new" });
      expect(useChatStore.getState().activeSession?.cwd).toBe("/new");
    });

    it("noop when no active session", async () => {
      let called = false;
      mockTauriCommand("update_chat_session_cwd", () => {
        called = true;
      });

      await useChatStore.getState().updateCwd("/x");
      expect(called).toBe(false);
    });
  });

  // ──────────────────────────────────────────────
  // Chat actions
  // ──────────────────────────────────────────────
  describe("sendMessage", () => {
    it("adds optimistic user message and starts streaming", async () => {
      mockTauriCommand("send_chat_message", () => "req-123");
      useChatStore.setState({ activeSessionId: 1 });

      await useChatStore.getState().sendMessage("hello");

      const state = useChatStore.getState();
      expect(state.isStreaming).toBe(true);
      expect(state.messages).toHaveLength(1);
      expect(state.messages[0].role).toBe("user");
      expect(state.messages[0].content_json).toContain("hello");
    });

    it("noop when no active session", async () => {
      let sent = false;
      mockTauriCommand("send_chat_message", () => {
        sent = true;
        return "id";
      });

      await useChatStore.getState().sendMessage("hi");
      expect(sent).toBe(false);
    });

    it("noop when text is empty and no attachments", async () => {
      let sent = false;
      mockTauriCommand("send_chat_message", () => {
        sent = true;
        return "id";
      });
      useChatStore.setState({ activeSessionId: 1 });

      await useChatStore.getState().sendMessage("   ");
      expect(sent).toBe(false);
    });

    it("allows empty text when attachments are provided", async () => {
      let sent = false;
      mockTauriCommand("send_chat_message", () => {
        sent = true;
        return "id";
      });
      useChatStore.setState({ activeSessionId: 1 });

      await useChatStore
        .getState()
        .sendMessage("", [{ media_type: "image/png", path: "/tmp/x.png" }]);
      expect(sent).toBe(true);
    });

    it("captures error message on send failure", async () => {
      mockTauriCommand("send_chat_message", () => {
        throw new Error("network");
      });
      useChatStore.setState({ activeSessionId: 1 });

      await useChatStore.getState().sendMessage("hi");

      expect(useChatStore.getState().isStreaming).toBe(false);
      expect(useChatStore.getState().error).toBe("network");
    });
  });

  describe("respondPermission", () => {
    it("calls IPC with toolName from pendingPermission and clears it", async () => {
      let received: unknown = null;
      mockTauriCommand("respond_chat_permission", (args) => {
        received = args;
      });

      useChatStore.setState({
        pendingPermission: {
          permissionId: "p1",
          toolName: "Edit",
          toolInput: {},
        },
      });

      await useChatStore.getState().respondPermission("p1", "allow", "session");

      const r = received as { permissionId: string; toolName: string };
      expect(r.permissionId).toBe("p1");
      expect(r.toolName).toBe("Edit");
      expect(useChatStore.getState().pendingPermission).toBeNull();
    });

    it("defaults scope to 'once' when omitted", async () => {
      let received: unknown = null;
      mockTauriCommand("respond_chat_permission", (args) => {
        received = args;
      });

      await useChatStore.getState().respondPermission("p1", "deny");
      expect((received as { scope: string }).scope).toBe("once");
    });

    it("logs but does not throw when IPC fails", async () => {
      mockTauriCommand("respond_chat_permission", () => {
        throw new Error("nope");
      });
      const errSpy = vi.spyOn(console, "error").mockImplementation(() => {});

      await useChatStore.getState().respondPermission("p1", "allow");

      expect(errSpy).toHaveBeenCalled();
      errSpy.mockRestore();
    });
  });

  describe("editAndResend / regenerate / resetAndContinue", () => {
    it("editAndResend deletes after turn and resends", async () => {
      let deleteArgs: unknown = null;
      mockTauriCommand("delete_messages_after", (args) => {
        deleteArgs = args;
      });
      mockTauriCommand("list_chat_messages", () => []);
      mockTauriCommand("send_chat_message", () => "req");

      useChatStore.setState({ activeSessionId: 1, messages: [] });

      await useChatStore.getState().editAndResend(2, "new text");

      expect(deleteArgs).toEqual({ sessionId: 1, turnIndex: 2 });
      expect(useChatStore.getState().messages).toHaveLength(1); // optimistic
    });

    it("regenerate noop when fewer than 2 messages", async () => {
      let called = false;
      mockTauriCommand("delete_messages_after", () => {
        called = true;
      });
      useChatStore.setState({
        activeSessionId: 1,
        messages: [mkMessage(1, 1, "user", "hi")],
      });

      await useChatStore.getState().regenerate();
      expect(called).toBe(false);
    });

    it("regenerate finds last user message, deletes after it, resends text", async () => {
      let deleteArgs: unknown = null;
      mockTauriCommand("delete_messages_after", (args) => {
        deleteArgs = args;
      });
      mockTauriCommand("list_chat_messages", () => []);
      mockTauriCommand("send_chat_message", () => "req");

      useChatStore.setState({
        activeSessionId: 1,
        messages: [
          mkMessage(1, 1, "user", "first", 0),
          mkMessage(2, 1, "assistant", "reply", 1),
          mkMessage(3, 1, "user", "second", 2),
          mkMessage(4, 1, "assistant", "reply2", 3),
        ],
      });

      await useChatStore.getState().regenerate();

      expect(deleteArgs).toEqual({ sessionId: 1, turnIndex: 3 });
    });

    it("resetAndContinue clears claude_id, resets resumeFailed, and resends last user message", async () => {
      let cleared = false;
      mockTauriCommand("clear_session_claude_id", () => {
        cleared = true;
      });
      mockTauriCommand("send_chat_message", () => "req");

      useChatStore.setState({
        activeSessionId: 1,
        resumeFailed: true,
        error: "boom",
        messages: [mkMessage(1, 1, "user", "do it", 0)],
      });

      await useChatStore.getState().resetAndContinue();

      expect(cleared).toBe(true);
      expect(useChatStore.getState().resumeFailed).toBe(false);
      expect(useChatStore.getState().error).toBeNull();
    });
  });

  // ──────────────────────────────────────────────
  // Listeners
  // ──────────────────────────────────────────────
  describe("setupChatListeners", () => {
    it("registers all 7 chat channels", () => {
      setupChatListeners();
      const channels = listen.mock.calls.map((c) => c[0]);
      expect(channels).toContain("chat:delta");
      expect(channels).toContain("chat:tool_use");
      expect(channels).toContain("chat:tool_result");
      expect(channels).toContain("chat:permission_request");
      expect(channels).toContain("chat:done");
      expect(channels).toContain("chat:error");
      expect(channels).toContain("chat:session-updated");
    });

    it("chat:delta accumulates streamingContent for active session", () => {
      setupChatListeners();
      useChatStore.setState({ activeSessionId: 1 });

      emitTauriEvent("chat:delta", { session_id: 1, text: "Hello " });
      emitTauriEvent("chat:delta", { session_id: 1, text: "world" });

      expect(useChatStore.getState().streamingContent).toBe("Hello world");
    });

    it("chat:delta ignores other sessions", () => {
      setupChatListeners();
      useChatStore.setState({ activeSessionId: 1 });

      emitTauriEvent("chat:delta", { session_id: 99, text: "stray" });

      expect(useChatStore.getState().streamingContent).toBe("");
    });

    it("chat:tool_use registers a pending tool activity", () => {
      setupChatListeners();
      useChatStore.setState({ activeSessionId: 1 });

      emitTauriEvent("chat:tool_use", {
        session_id: 1,
        tool_use_id: "tu-1",
        name: "Bash",
        input: { command: "ls" },
      });

      const activity = useChatStore.getState().toolActivity.get("tu-1");
      expect(activity?.pending).toBe(true);
      expect(activity?.name).toBe("Bash");
    });

    it("chat:tool_result completes a pending activity with result text", () => {
      setupChatListeners();
      useChatStore.setState({ activeSessionId: 1 });

      emitTauriEvent("chat:tool_use", {
        session_id: 1,
        tool_use_id: "tu-2",
        name: "Read",
        input: {},
      });
      emitTauriEvent("chat:tool_result", {
        session_id: 1,
        tool_use_id: "tu-2",
        content: [{ type: "text", text: "file contents" }],
        is_error: false,
      });

      const activity = useChatStore.getState().toolActivity.get("tu-2");
      expect(activity?.pending).toBe(false);
      expect(activity?.result).toContain("file contents");
      expect(activity?.isError).toBe(false);
    });

    it("chat:permission_request stores pendingPermission", () => {
      setupChatListeners();
      useChatStore.setState({ activeSessionId: 1 });

      emitTauriEvent("chat:permission_request", {
        session_id: 1,
        permission_id: "perm-1",
        tool_name: "Edit",
        tool_input: { path: "x.md" },
      });

      const pending = useChatStore.getState().pendingPermission;
      expect(pending?.permissionId).toBe("perm-1");
      expect(pending?.toolName).toBe("Edit");
    });

    it("chat:done resets streaming and reloads messages", async () => {
      setupChatListeners();
      useChatStore.setState({
        activeSessionId: 1,
        isStreaming: true,
        streamingContent: "partial",
      });
      mockTauriCommand("list_chat_messages", () => [
        mkMessage(1, 1, "user", "hi"),
      ]);

      emitTauriEvent("chat:done", {
        session_id: 1,
        usage: null,
        stop_reason: "end_turn",
      });
      // Allow async listChatMessages to flush
      await Promise.resolve();
      await Promise.resolve();

      const state = useChatStore.getState();
      expect(state.isStreaming).toBe(false);
      expect(state.streamingContent).toBe("");
    });

    it("chat:error category 'auth' produces friendly message", () => {
      setupChatListeners();
      useChatStore.setState({ activeSessionId: 1, isStreaming: true });
      mockTauriCommand("list_chat_messages", () => []);

      emitTauriEvent("chat:error", {
        session_id: 1,
        category: "auth",
        message: "raw",
      });

      const state = useChatStore.getState();
      expect(state.error).toContain("Authentication");
      expect(state.isStreaming).toBe(false);
    });

    it("chat:error category 'rate_limit' produces friendly message", () => {
      setupChatListeners();
      useChatStore.setState({ activeSessionId: 1 });
      mockTauriCommand("list_chat_messages", () => []);

      emitTauriEvent("chat:error", {
        session_id: 1,
        category: "rate_limit",
        message: "raw",
      });

      expect(useChatStore.getState().error).toContain("Rate limit");
    });

    it("chat:error category 'resume_failed' sets resumeFailed flag", () => {
      setupChatListeners();
      useChatStore.setState({ activeSessionId: 1 });
      mockTauriCommand("list_chat_messages", () => []);

      emitTauriEvent("chat:error", {
        session_id: 1,
        category: "resume_failed",
        message: "raw",
      });

      expect(useChatStore.getState().resumeFailed).toBe(true);
    });

    it("chat:error with unknown category passes raw message through", () => {
      setupChatListeners();
      useChatStore.setState({ activeSessionId: 1 });
      mockTauriCommand("list_chat_messages", () => []);

      emitTauriEvent("chat:error", {
        session_id: 1,
        category: "weird",
        message: "the actual message",
      });

      expect(useChatStore.getState().error).toBe("the actual message");
    });

    it("chat:session-updated triggers refreshSessions", async () => {
      mockTauriCommand("list_chat_sessions", () => [mkSession(1, "Updated")]);
      useChatStore.setState({ activeSessionId: 1 });

      setupChatListeners();
      emitTauriEvent("chat:session-updated", null);
      await Promise.resolve();
      await Promise.resolve();

      expect(useChatStore.getState().activeSession?.title).toBe("Updated");
    });

    it("listeners ignore events with wrong session_id", () => {
      setupChatListeners();
      useChatStore.setState({ activeSessionId: 1 });

      emitTauriEvent("chat:tool_use", {
        session_id: 999,
        tool_use_id: "tu-x",
        name: "Bash",
        input: {},
      });

      expect(useChatStore.getState().toolActivity.has("tu-x")).toBe(false);
    });
  });

  // ──────────────────────────────────────────────
  // setupSessionWatcher
  // ──────────────────────────────────────────────
  describe("setupSessionWatcher", () => {
    it("loads messages when activeSessionId changes to a non-null value", async () => {
      mockTauriCommand("list_chat_messages", () => [
        mkMessage(1, 5, "user", "hi"),
      ]);

      setupSessionWatcher();
      useChatStore.setState({ activeSessionId: 5 });

      await Promise.resolve();
      await Promise.resolve();

      expect(useChatStore.getState().messages).toHaveLength(1);
    });

    it("clears state when activeSessionId becomes null", async () => {
      // First trigger non-null to set prevSessionId
      mockTauriCommand("list_chat_messages", () => []);
      setupSessionWatcher();
      useChatStore.setState({ activeSessionId: 7 });
      await Promise.resolve();

      // Now nullify
      useChatStore.setState({
        activeSessionId: null,
        messages: [mkMessage(1, 7, "user", "x")],
        isStreaming: true,
      });

      const state = useChatStore.getState();
      expect(state.messages).toEqual([]);
      expect(state.isStreaming).toBe(false);
    });
  });
});
