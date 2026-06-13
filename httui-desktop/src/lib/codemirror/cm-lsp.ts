// LSP-backed language features for the markdown editor: diagnostics
// (squiggles via the client's serverDiagnostics extension), hover
// tooltips and completion served by httui-lsp. The local `{{` ref
// autocomplete and hover remain active alongside this extension while
// the language server path stabilizes; the older path is removed once
// the server is the proven source.
import type { Extension } from "@codemirror/state";
import { keymap } from "@codemirror/view";
import {
  hoverTooltips,
  jumpToDefinitionKeymap,
  findReferencesKeymap,
  renameKeymap,
} from "@codemirror/lsp-client";
import { useWorkspaceStore } from "@/stores/workspace";
import { fileUri, getLspClient } from "@/lib/lsp/client";

export function createLspExtension(filePath: string): Extension[] {
  const vaultPath = useWorkspaceStore.getState().vaultPath;
  if (!vaultPath) return [];
  const client = getLspClient();
  return [
    client.plugin(fileUri(vaultPath, filePath), "markdown"),
    // completion is wired through the editor's autocompletion override
    // (markdown-extensions.ts) — `serverCompletion()` here would add a
    // second autocompletion config that the override silences anyway
    hoverTooltips(),
    // alias navigation served by httui-lsp: F12 go-to-definition,
    // Shift-F12 find-references (with the client's reference panel),
    // F2 rename. Resolved against the block-alias grammar, so they never
    // act on text that merely looks like an alias.
    keymap.of([
      ...jumpToDefinitionKeymap,
      ...findReferencesKeymap,
      ...renameKeymap,
    ]),
  ];
}
