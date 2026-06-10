// Tauri transport for @codemirror/lsp-client. The webview cannot speak
// stdio, so outgoing messages cross as `lsp_send` invokes and incoming
// ones arrive as `lsp:message` events (the Rust side owns the
// Content-Length framing). Messages sent before the sidecar finishes
// starting are queued and flushed in order.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Transport } from "@codemirror/lsp-client";

export function createTauriLspTransport(): Transport {
  const handlers = new Set<(value: string) => void>();
  const queue: string[] = [];
  let started = false;
  let starting: Promise<void> | null = null;

  void listen<string>("lsp:message", (event) => {
    for (const handler of handlers) handler(event.payload);
  });
  void listen("lsp:exit", () => {
    started = false;
    starting = null;
  });

  const ensureStarted = () => {
    if (!starting) {
      starting = invoke<void>("lsp_start")
        .then(() => {
          started = true;
          while (queue.length > 0) {
            const message = queue.shift();
            if (message !== undefined) {
              void invoke("lsp_send", { message });
            }
          }
        })
        .catch((e) => {
          starting = null;
          console.error("[lsp] failed to start language server:", e);
        });
    }
    return starting;
  };

  return {
    send(message: string) {
      if (started) {
        void invoke("lsp_send", { message });
      } else {
        queue.push(message);
        void ensureStarted();
      }
    },
    subscribe(handler) {
      handlers.add(handler);
    },
    unsubscribe(handler) {
      handlers.delete(handler);
    },
  };
}
