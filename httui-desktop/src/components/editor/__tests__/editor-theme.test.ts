import { describe, it, expect } from "vitest";

import { editorTheme } from "@/components/editor/editor-theme";

describe("editorTheme", () => {
  it("is a CodeMirror Extension (truthy + non-array consumable)", () => {
    expect(editorTheme).toBeTruthy();
    // CM6 Extension is a Facet/StateField/[]; the constant is exported as
    // an object reference suitable for inclusion in EditorState.create()'s
    // `extensions` array.
    expect(typeof editorTheme).toBe("object");
  });

  it("is a stable single-instance reference (Emotion caching guarantee)", async () => {
    const reimport = (await import("@/components/editor/editor-theme"))
      .editorTheme;
    expect(reimport).toBe(editorTheme);
  });
});
