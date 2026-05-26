export interface ChatCommand {
  type: "chat";
  request_id: string;
  claude_session_id: string | null;
  cwd: string | null;
  allowed_tools: string[];
  content: ContentBlock[];
}

export interface PermissionResponseCommand {
  type: "permission_response";
  permission_id: string;
  decision: {
    behavior: "allow" | "deny";
    message?: string;
  };
}

export interface AbortCommand {
  type: "abort";
  request_id: string;
}

export interface PingCommand {
  type: "ping";
}

export type IncomingCommand =
  | ChatCommand
  | PermissionResponseCommand
  | AbortCommand
  | PingCommand;

export interface SessionEvent {
  type: "session";
  request_id: string;
  claude_session_id: string;
}

export interface TextDeltaEvent {
  type: "text_delta";
  request_id: string;
  text: string;
}

export interface ToolUseEvent {
  type: "tool_use";
  request_id: string;
  tool_use_id: string;
  name: string;
  input: Record<string, unknown>;
}

export interface ToolResultEvent {
  type: "tool_result";
  request_id: string;
  tool_use_id: string;
  content: ContentBlock[];
  is_error: boolean;
}

export interface PermissionRequestEvent {
  type: "permission_request";
  request_id: string;
  permission_id: string;
  tool_name: string;
  tool_input: Record<string, unknown>;
}

export interface DoneEvent {
  type: "done";
  request_id: string;
  usage: {
    input_tokens: number;
    output_tokens: number;
    cache_read_tokens: number;
  } | null;
  stop_reason: string | null;
}

export interface ErrorEvent {
  type: "error";
  request_id: string;
  category: "auth" | "rate_limit" | "network" | "invalid_input" | "internal";
  message: string;
}

export interface PongEvent {
  type: "pong";
}

export type OutgoingEvent =
  | SessionEvent
  | TextDeltaEvent
  | ToolUseEvent
  | ToolResultEvent
  | PermissionRequestEvent
  | DoneEvent
  | ErrorEvent
  | PongEvent;

export type ContentBlock =
  | { type: "text"; text: string }
  | {
      type: "image";
      source: {
        type: "base64";
        media_type: string;
        data: string;
      };
    };

import { createHmac } from "node:crypto";

const HMAC_SECRET = process.env.SIDECAR_HMAC_SECRET ?? "";

function computeHmac(payload: string): string {
  return createHmac("sha256", HMAC_SECRET).update(payload).digest("hex");
}

function verifyHmac(payload: string, expectedHmac: string): boolean {
  const computed = computeHmac(payload);
  if (computed.length !== expectedHmac.length) return false; // constant-time length check
  let result = 0;
  for (let i = 0; i < computed.length; i++) {
    result |= computed.charCodeAt(i) ^ expectedHmac.charCodeAt(i);
  }
  return result === 0;
}

export function send(event: OutgoingEvent): void {
  const payload = JSON.stringify(event);
  if (HMAC_SECRET) {
    const hmac = computeHmac(payload);
    process.stdout.write(JSON.stringify({ hmac, payload }) + "\n");
  } else {
    process.stdout.write(payload + "\n");
  }
}

export function log(...args: unknown[]): void {
  process.stderr.write(`[sidecar] ${args.join(" ")}\n`);
}

export function parseCommand(line: string): IncomingCommand | null {
  try {
    const raw = JSON.parse(line);
    if (raw.hmac && raw.payload && HMAC_SECRET) {
      // payload is a JSON string — verify directly without re-serialization
      const payloadStr =
        typeof raw.payload === "string"
          ? raw.payload
          : JSON.stringify(raw.payload);
      if (!verifyHmac(payloadStr, raw.hmac)) {
        log("HMAC verification failed — dropping message");
        return null;
      }
      return (
        typeof raw.payload === "string" ? JSON.parse(raw.payload) : raw.payload
      ) as IncomingCommand;
    }
    return raw as IncomingCommand; // no envelope — backward compat
  } catch {
    log("Failed to parse command:", line);
    return null;
  }
}
