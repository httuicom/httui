// The `{{ref}}` visual highlight is the only local ref affordance left
// in the document path (hover/completion live in the language server).
import { describe, expect, it } from "vitest";
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

import { referenceHighlight } from "../cm-references";
import { setSecretEnvKeys } from "../secret-env-keys";

function mount(doc: string) {
  const parent = document.createElement("div");
  document.body.appendChild(parent);
  return new EditorView({
    state: EditorState.create({ doc, extensions: [referenceHighlight] }),
    parent,
  });
}

describe("referenceHighlight", () => {
  it("marks {{ref}} spans and tracks document edits", () => {
    const view = mount("GET /x?a={{req1.response.body.id}}");
    expect(
      view.dom.querySelectorAll(".cm-reference-highlight").length,
    ).toBeGreaterThan(0);

    view.dispatch({
      changes: { from: view.state.doc.length, insert: " {{B}}" },
    });
    expect(
      view.dom.querySelectorAll(".cm-reference-highlight").length,
    ).toBeGreaterThanOrEqual(2);
    view.destroy();
  });

  it("gives grammar tokens their own classes", () => {
    const view = mount("{{req1.response.results.0.id}} {{$prev.body}}");
    expect(view.dom.querySelectorAll(".cm-ref-name").length).toBe(1);
    expect(view.dom.querySelectorAll(".cm-ref-prev").length).toBe(1);
    expect(view.dom.querySelectorAll(".cm-ref-index").length).toBe(1);
    view.destroy();
  });

  it("still highlights a half-typed ref's parsed prefix", () => {
    const view = mount("{{req1.respo");
    // error recovery keeps the alias token painted even mid-typing
    expect(view.dom.querySelectorAll(".cm-ref-name").length).toBe(1);
    view.destroy();
  });

  it("leaves plain text and single braces unmarked", () => {
    const view = mount("plain { not a ref }");
    expect(view.dom.querySelectorAll(".cm-reference-highlight").length).toBe(0);
    view.destroy();
  });

  describe("secret env var marking", () => {
    it("marks a bare {{KEY}} whose key is a secret env var", () => {
      setSecretEnvKeys(["TOKEN"]);
      const view = mount("Authorization: Bearer {{TOKEN}}");
      expect(view.dom.querySelectorAll(".cm-ref-secret").length).toBe(1);
      view.destroy();
      setSecretEnvKeys([]);
    });

    it("does not mark a non-secret bare key", () => {
      setSecretEnvKeys(["TOKEN"]);
      const view = mount("{{BASE_URL}}");
      expect(view.dom.querySelectorAll(".cm-ref-secret").length).toBe(0);
      view.destroy();
      setSecretEnvKeys([]);
    });

    it("does not mark a ref that has a path (block ref, not env var)", () => {
      setSecretEnvKeys(["TOKEN"]);
      // even if a path head collides with a secret key name, a dotted ref
      // is a block reference, not the env var
      const view = mount("{{TOKEN.body.id}}");
      expect(view.dom.querySelectorAll(".cm-ref-secret").length).toBe(0);
      view.destroy();
      setSecretEnvKeys([]);
    });

    it("repaints when the secret set lands AFTER the editor mounted", () => {
      // The set is populated asynchronously (post-IPC), usually after the
      // editor has already painted. Without a forced rebuild the highlight
      // would never appear until the next edit — the actual shipped bug.
      const view = mount("Authorization: Bearer {{TOKEN}}");
      expect(view.dom.querySelectorAll(".cm-ref-secret").length).toBe(0);

      setSecretEnvKeys(["TOKEN"]);
      expect(view.dom.querySelectorAll(".cm-ref-secret").length).toBe(1);

      view.destroy();
      setSecretEnvKeys([]);
    });
  });
});
