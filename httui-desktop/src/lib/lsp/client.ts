// Singleton LSP client for the httui language server. Created lazily on
// first editor mount; all editors share the connection (one server per
// app process, one workspace).
import { LSPClient, serverDiagnostics } from "@codemirror/lsp-client";
import { createTauriLspTransport } from "./transport";

let client: LSPClient | null = null;

export function getLspClient(): LSPClient {
  if (!client) {
    client = new LSPClient({
      extensions: [serverDiagnostics()],
    }).connect(createTauriLspTransport());
  }
  return client;
}

/** Test seam: drop the singleton so each test builds a fresh client. */
export function resetLspClient() {
  client = null;
}

export function fileUri(vaultPath: string, filePath: string): string {
  const joined = `${vaultPath}/${filePath}`.replace(/\/+/g, "/");
  return `file://${encodeURI(joined)}`;
}
