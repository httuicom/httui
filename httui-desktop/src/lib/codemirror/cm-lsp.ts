// LSP-backed language features for the markdown editor: diagnostics
// (squiggles via the client's serverDiagnostics extension), hover
// tooltips and completion served by httui-lsp. The local `{{` ref
// autocomplete and hover remain active alongside this extension while
// the language server path stabilizes; the older path is removed once
// the server is the proven source.
import type { Extension } from "@codemirror/state";
import { hoverTooltips, serverCompletion } from "@codemirror/lsp-client";
import { useWorkspaceStore } from "@/stores/workspace";
import { fileUri, getLspClient } from "@/lib/lsp/client";

export function createLspExtension(filePath: string): Extension[] {
  const vaultPath = useWorkspaceStore.getState().vaultPath;
  if (!vaultPath) return [];
  const client = getLspClient();
  return [
    client.plugin(fileUri(vaultPath, filePath), "markdown"),
    serverCompletion(),
    hoverTooltips(),
  ];
}
