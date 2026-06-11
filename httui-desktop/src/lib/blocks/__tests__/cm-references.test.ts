// The `{{ref}}` visual highlight is the only local ref affordance left
// in the document path (hover/completion live in the language server).
import { describe, expect, it } from "vitest";
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

import { referenceHighlight } from "../cm-references";

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

  it("leaves plain text and single braces unmarked", () => {
    const view = mount("plain { not a ref }");
    expect(view.dom.querySelectorAll(".cm-reference-highlight").length).toBe(0);
    view.destroy();
  });
});
