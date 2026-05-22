import { createInterface } from "readline";
import { parseCommand, send, log } from "./protocol.js";
import {
  handleChat,
  handleAbort,
  handlePermissionResponse,
} from "./handlers/chat.js";

// Force usage via Claude Max subscription — never use API key directly
delete process.env.ANTHROPIC_API_KEY;
delete process.env.ANTHROPIC_AUTH_TOKEN;

log("Starting claude-sidecar");

const rl = createInterface({
  input: process.stdin,
  crlfDelay: Infinity,
});

rl.on("line", (line: string) => {
  if (!line.trim()) return;

  const cmd = parseCommand(line);
  if (!cmd) return;

  switch (cmd.type) {
    case "chat":
      handleChat(cmd).catch((err) => {
        log("Unhandled error in chat handler:", err);
      });
      break;

    case "abort":
      handleAbort(cmd.request_id);
      break;

    case "permission_response":
      handlePermissionResponse(cmd);
      break;

    case "ping":
      send({ type: "pong" });
      break;

    default:
      log("Unknown command type:", (cmd as { type: string }).type);
  }
});

rl.on("close", () => {
  log("stdin closed, exiting");
  process.exit(0);
});

process.on("SIGINT", () => {
  log("Received SIGINT, exiting");
  process.exit(0);
});

process.on("SIGTERM", () => {
  log("Received SIGTERM, exiting");
  process.exit(0);
});
