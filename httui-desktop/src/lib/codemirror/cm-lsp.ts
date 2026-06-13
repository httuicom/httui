// LSP-backed language features for the markdown editor: diagnostics
// (squiggles via the client's serverDiagnostics extension), hover
// tooltips and completion served by httui-lsp. The local `{{` ref
// autocomplete and hover remain active alongside this extension while
// the language server path stabilizes; the older path is removed once
// the server is the proven source.
import type { Extension } from "@codemirror/state";
import { keymap, EditorView } from "@codemirror/view";
import {
  hoverTooltips,
  jumpToDefinition,
  findReferences,
  renameSymbol,
  closeReferencePanel,
} from "@codemirror/lsp-client";
import { useWorkspaceStore } from "@/stores/workspace";
import { fileUri, getLspClient } from "@/lib/lsp/client";

// Bind the lsp-client commands to our own chords instead of its default
// keymaps (F12/F2/Shift-F12) — F-keys are not used as defaults here, and
// Ctrl is free on macOS (Cmd is the modifier for the app globals).
const navKeymap = keymap.of([
  { key: "Ctrl-]", run: jumpToDefinition },
  { key: "Ctrl-Shift-]", run: findReferences },
  { key: "Ctrl-Shift-r", run: renameSymbol },
  { key: "Escape", run: closeReferencePanel },
]);

// Mod+click (Cmd on macOS, Ctrl elsewhere) on a ref jumps to its
// definition — move the caret to the click, then run the command.
const modClickDefinition = EditorView.domEventHandlers({
  mousedown(event, view) {
    if (!(event.metaKey || event.ctrlKey)) return false;
    const pos = view.posAtCoords({ x: event.clientX, y: event.clientY });
    if (pos == null) return false;
    view.dispatch({ selection: { anchor: pos } });
    jumpToDefinition(view);
    event.preventDefault();
    return true;
  },
});

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
    // alias navigation served by httui-lsp, resolved against the
    // block-alias grammar (never acts on text that merely looks like an
    // alias): Ctrl-] go-to-definition (+ Mod-click), Ctrl-Shift-]
    // find-references, Ctrl-Shift-r rename.
    navKeymap,
    modClickDefinition,
  ];
}
