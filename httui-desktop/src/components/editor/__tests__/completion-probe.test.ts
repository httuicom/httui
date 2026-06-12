import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { currentCompletions } from "@codemirror/autocomplete";

import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";
import { emitTauriEvent, clearTauriListeners } from "@/test/mocks/tauri-event";
import { useWorkspaceStore } from "@/stores/workspace";
import { resetLspClient } from "@/lib/lsp/client";
import { createLspExtension } from "@/lib/codemirror/cm-lsp";
import { buildExtensions } from "@/components/editor/markdown-extensions";

const DOC = "```http alias=req1\nGET https://x.dev/{\n```\n";

function installFakeLspServer() {
  mockTauriCommand("lsp_start", () => {});
  mockTauriCommand("lsp_send", (args) => {
    const m = JSON.parse((args as { message: string }).message);
    const reply = (msg: object) =>
      setTimeout(() => emitTauriEvent("lsp:message", JSON.stringify(msg)), 0);
    if (m.method === "initialize") {
      reply({
        jsonrpc: "2.0",
        id: m.id,
        result: {
          capabilities: {
            textDocumentSync: { openClose: true, change: 1 },
            completionProvider: { triggerCharacters: ["{"] },
            hoverProvider: true,
          },
        },
      });
    } else if (m.method === "textDocument/completion") {
      reply({ jsonrpc: "2.0", id: m.id, result: [{ label: "req1", kind: 6 }] });
    } else if (m.id !== undefined) {
      reply({ jsonrpc: "2.0", id: m.id, result: null });
    }
  });
}

beforeEach(() => {
  resetLspClient();
  installFakeLspServer();
  useWorkspaceStore.setState({ vaultPath: "/tmp/vault" });
});

afterEach(() => {
  clearTauriMocks();
  clearTauriListeners();
  useWorkspaceStore.setState({ vaultPath: null });
});

describe("completion inside http fence (full editor extensions)", () => {
  it("typing the ref opener offers server completions", async () => {
    const parent = document.createElement("div");
    document.body.appendChild(parent);
    const pos = DOC.indexOf("{") + 1;
    const view = new EditorView({
      state: EditorState.create({
        doc: DOC,
        selection: { anchor: pos },
        extensions: [
          ...buildExtensions({
            filePath: "notes/a.md",
            entriesRef: { current: [] },
            handleFileSelectRef: { current: () => {} },
            docHeaderHandle: null,
          }),
          ...createLspExtension("notes/a.md"),
        ],
      }),
      parent,
    });
    await new Promise((r) => setTimeout(r, 100));
    view.dispatch({
      changes: { from: pos, insert: "{" },
      selection: { anchor: pos + 1 },
      userEvent: "input.type",
    });
    await new Promise((r) => setTimeout(r, 500));
    expect(currentCompletions(view.state).map((c) => c.label)).toContain(
      "req1",
    );
    view.destroy();
  });
});
