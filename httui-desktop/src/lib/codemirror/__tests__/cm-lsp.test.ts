import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";
import { clearTauriListeners } from "@/test/mocks/tauri-event";
import { useWorkspaceStore } from "@/stores/workspace";
import { resetLspClient } from "@/lib/lsp/client";
import { createLspExtension } from "../cm-lsp";

beforeEach(() => {
  resetLspClient();
  mockTauriCommand("lsp_start", () => {});
  mockTauriCommand("lsp_send", () => {});
});

afterEach(() => {
  clearTauriMocks();
  clearTauriListeners();
  useWorkspaceStore.setState({ vaultPath: null });
});

describe("createLspExtension", () => {
  it("is empty without an active vault", () => {
    useWorkspaceStore.setState({ vaultPath: null });
    expect(createLspExtension("notes/a.md")).toEqual([]);
  });

  it("produces the plugin, hover and navigation extensions for a vault file", () => {
    useWorkspaceStore.setState({ vaultPath: "/tmp/vault" });
    const exts = createLspExtension("notes/a.md");
    // client.plugin + hoverTooltips + nav keymap + Mod-click definition
    expect(exts.length).toBe(4);
    for (const ext of exts) {
      expect(ext).toBeTruthy();
    }
  });

  describe("Mod-click go-to-definition handler", () => {
    let view: EditorView;
    beforeEach(() => useWorkspaceStore.setState({ vaultPath: "/tmp/vault" }));
    afterEach(() => view?.destroy());

    function mount() {
      view = new EditorView({
        state: EditorState.create({
          doc: "```http alias=req1\nGET /{{req1.id}}\n```",
          extensions: createLspExtension("notes/a.md"),
        }),
        parent: document.body,
      });
      return view;
    }

    const click = (mod: boolean) =>
      view.contentDOM.dispatchEvent(
        new MouseEvent("mousedown", {
          bubbles: true,
          cancelable: true,
          metaKey: mod,
          clientX: 1,
          clientY: 1,
        }),
      );

    it("ignores a plain click (no mod key)", () => {
      mount();
      view.dispatch({ selection: { anchor: 0 } });
      click(false);
      expect(view.state.selection.main.anchor).toBe(0);
    });

    it("moves the caret to the clicked position on Mod-click", () => {
      mount();
      // jsdom has no layout, so pin a resolvable position
      view.posAtCoords = (() => 24) as EditorView["posAtCoords"];
      view.dispatch({ selection: { anchor: 0 } });
      click(true);
      expect(view.state.selection.main.anchor).toBe(24);
    });

    it("no-ops on Mod-click over an unresolvable position", () => {
      mount();
      view.posAtCoords = (() => null) as unknown as EditorView["posAtCoords"];
      // the handler bails (returns false) without jumping or throwing
      expect(() => click(true)).not.toThrow();
    });
  });
});
