import {
  query,
  type Query,
  type SDKMessage,
} from "@anthropic-ai/claude-agent-sdk";
import type { ChatCommand, PermissionResponseCommand } from "../protocol.js";
import { send, log } from "../protocol.js";

const activeQueries = new Map<string, Query>();

const pendingPermissions = new Map<
  string,
  {
    resolve: (decision: PermissionResponseCommand["decision"]) => void;
  }
>();

let permissionCounter = 0;

export async function handleChat(cmd: ChatCommand): Promise<void> {
  const { request_id, claude_session_id, cwd, allowed_tools, content } = cmd;

  try {
    const hasImages = content.some((b) => b.type === "image");
    const textParts = content
      .filter((b): b is { type: "text"; text: string } => b.type === "text")
      .map((b) => b.text);
    const textPrompt = textParts.join("\n");

    if (!textPrompt.trim() && content.length === 0) {
      send({
        type: "error",
        request_id,
        category: "invalid_input",
        message: "Empty message content",
      });
      return;
    }

    // Images must be sent as SDKUserMessage (async iterable) so the API receives content blocks.
    let prompt:
      | string
      | AsyncIterable<import("@anthropic-ai/claude-agent-sdk").SDKUserMessage>;
    if (hasImages) {
      const messageContent = content.map((block) => {
        if (block.type === "text") {
          return { type: "text" as const, text: block.text };
        }
        return {
          type: "image" as const,
          source: block.source,
        };
      });

      async function* singleMessage() {
        yield {
          type: "user" as const,
          message: {
            role: "user" as const,
            content: messageContent,
          },
          parent_tool_use_id: null,
          session_id: "",
        };
      }
      prompt = singleMessage();
    } else {
      prompt = textPrompt;
    }

    const mcpServers: Record<string, { command: string; args: string[] }> = {};
    const effectiveCwd = cwd || process.cwd();
    log("cwd:", cwd, "effectiveCwd:", effectiveCwd);
    {
      const path = await import("path");
      const candidates = [
        path.resolve(process.cwd(), "target/debug/httui-mcp"),
        path.resolve(process.cwd(), "../target/debug/httui-mcp"),
        path.resolve(
          import.meta.dirname ?? ".",
          "../../target/debug/httui-mcp",
        ),
      ];
      const fs = await import("fs");
      const mcpBinary = candidates.find((p) => fs.existsSync(p));
      if (mcpBinary) {
        const os = await import("os");
        const home = os.homedir();
        const dbDir = path.join(
          home,
          "Library/Application Support/com.notes.app",
        );
        log("Using MCP binary:", mcpBinary, "db:", dbDir);
        mcpServers["httui_notes"] = {
          command: mcpBinary,
          args: ["--vault", effectiveCwd, "--db", dbDir],
        };
      } else {
        log("MCP binary not found, tried:", candidates.join(", "));
      }
    }

    const q = query({
      prompt,
      options: {
        ...(process.env.CLAUDE_CLI_PATH
          ? { pathToClaudeCodeExecutable: process.env.CLAUDE_CLI_PATH }
          : {}),
        ...(claude_session_id ? { resume: claude_session_id } : {}),
        ...(cwd ? { cwd } : {}),
        allowedTools: allowed_tools,
        permissionMode: "default",
        includePartialMessages: true,
        systemPrompt: {
          type: "preset" as const,
          preset: "claude_code" as const,
          append: `
You are inside the httui_notes desktop app — a markdown editor with executable blocks (HTTP requests, DB queries, E2E tests).

IMPORTANT: You have access to an MCP server called "httui_notes" with these tools:
- list_connections: List all database connections
- list_environments / get_env_variables / set_active_environment: Manage environments
- list_notes / read_note / create_note / update_note / search_notes: Manage vault notes
- list_blocks / execute_block: List and run executable blocks in notes
- get_schema: Get database schema for a connection
- test_connection: Test database connectivity

ALWAYS prefer these MCP tools over generic file system tools (Read, Grep, Bash) when the user asks about:
- Database connections, schemas, or queries → use list_connections, get_schema, test_connection
- Environment variables → use list_environments, get_env_variables
- Notes content → use list_notes, read_note, search_notes
- Running blocks → use execute_block

Only fall back to file system tools if the MCP tools cannot accomplish the task.
`.trim(),
        },
        ...(Object.keys(mcpServers).length > 0 ? { mcpServers } : {}),
        canUseTool: async (toolName, toolInput, { signal }) => {
          const permissionId = `perm_${++permissionCounter}`;

          send({
            type: "permission_request",
            request_id,
            permission_id: permissionId,
            tool_name: toolName,
            tool_input: toolInput,
          });

          const decision = await new Promise<
            PermissionResponseCommand["decision"]
          >((resolve, reject) => {
            pendingPermissions.set(permissionId, { resolve });
            signal.addEventListener("abort", () => {
              pendingPermissions.delete(permissionId);
              reject(new Error("Aborted"));
            });
          });

          if (decision.behavior === "allow") {
            return {
              behavior: "allow" as const,
              updatedInput: toolInput,
            };
          } else {
            return {
              behavior: "deny" as const,
              message: decision.message ?? "Denied by user",
            };
          }
        },
      },
    });

    activeQueries.set(request_id, q);

    let sessionEmitted = false;
    let lastPartialText = "";

    for await (const msg of q) {
      // Track partial text for delta extraction
      if (msg.type === "stream_event") {
        const event = msg.event;
        if (event.type === "content_block_delta" && "delta" in event) {
          const delta = event.delta as { type: string; text?: string };
          if (delta.type === "text_delta" && delta.text) {
            lastPartialText += delta.text; // accumulated for partial persistence
            send({
              type: "text_delta",
              request_id,
              text: delta.text,
            });
          }
        }
        continue;
      }

      if (msg.type === "system" && msg.subtype === "init") {
        if (!sessionEmitted) {
          send({
            type: "session",
            request_id,
            claude_session_id: msg.session_id,
          });
          sessionEmitted = true;
        }
        continue;
      }

      if (msg.type === "assistant") {
        const content = msg.message?.content ?? [];
        for (const block of content) {
          if (block.type === "tool_use") {
            send({
              type: "tool_use",
              request_id,
              tool_use_id: block.id,
              name: block.name,
              input: block.input as Record<string, unknown>,
            });
          }
        }
        continue;
      }

      if (msg.type === "user") {
        const content = msg.message?.content;
        if (Array.isArray(content)) {
          for (const block of content) {
            if (
              typeof block === "object" &&
              block !== null &&
              "type" in block &&
              block.type === "tool_result" &&
              "tool_use_id" in block
            ) {
              const toolResult = block as {
                type: "tool_result";
                tool_use_id: string;
                content?: unknown;
                is_error?: boolean;
              };
              send({
                type: "tool_result",
                request_id,
                tool_use_id: toolResult.tool_use_id,
                content: Array.isArray(toolResult.content)
                  ? toolResult.content
                  : [
                      {
                        type: "text",
                        text: String(toolResult.content ?? ""),
                      },
                    ],
                is_error: toolResult.is_error ?? false,
              });
            }
          }
        }
        continue;
      }

      if (msg.type === "result") {
        const usage = msg.usage;
        send({
          type: "done",
          request_id,
          usage: {
            input_tokens: usage.input_tokens ?? 0,
            output_tokens: usage.output_tokens ?? 0,
            cache_read_tokens: usage.cache_read_input_tokens ?? 0,
          },
          stop_reason: msg.subtype === "success" ? "end_turn" : "error",
        });
        continue;
      }
    }
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);

    let category:
      | "auth"
      | "rate_limit"
      | "network"
      | "resume_failed"
      | "internal" = "internal";
    if (message.includes("auth") || message.includes("login")) {
      category = "auth";
    } else if (message.includes("rate") || message.includes("429")) {
      category = "rate_limit";
    } else if (
      message.includes("network") ||
      message.includes("ECONNREFUSED")
    ) {
      category = "network";
    } else if (
      message.includes("session") &&
      (message.includes("not found") ||
        message.includes("expired") ||
        message.includes("invalid"))
    ) {
      category = "resume_failed";
    }

    send({ type: "error", request_id, category, message });
  } finally {
    activeQueries.delete(request_id);
  }
}

export function handleAbort(requestId: string): void {
  const q = activeQueries.get(requestId);
  if (q) {
    q.interrupt().catch(() => {});
    activeQueries.delete(requestId);
    log(`Interrupted request ${requestId}`);
  }
}

export function handlePermissionResponse(cmd: PermissionResponseCommand): void {
  log(
    `handlePermissionResponse id=${cmd.permission_id} behavior=${cmd.decision?.behavior} pending=${pendingPermissions.has(cmd.permission_id)}`,
  );
  const pending = pendingPermissions.get(cmd.permission_id);
  if (pending) {
    pending.resolve(cmd.decision);
    pendingPermissions.delete(cmd.permission_id);
    log(`Permission ${cmd.permission_id} resolved`);
  } else {
    log(`No pending permission for ${cmd.permission_id}`);
  }
}
