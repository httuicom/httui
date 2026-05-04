import { describe, expect, it, vi } from "vitest";

import {
  buildDocHeaderCallbacks,
  type CallbackDeps,
} from "@/components/editor/doc-header-callbacks";
import type { DocHeaderEntry } from "@/lib/codemirror/cm-doc-header";
import type { EditorView } from "@codemirror/view";

interface FakeView {
  state: { doc: { toString: () => string } };
}

function makeFakeView(content: string): EditorView {
  return {
    state: { doc: { toString: () => content } },
  } as unknown as EditorView;
}

function makeEntry(
  over: Partial<DocHeaderEntry> = {},
): DocHeaderEntry {
  return {
    id: "i1",
    container: document.createElement("div"),
    hasFrontmatter: false,
    view: null,
    titleInput: null,
    lastBodyOffset: 0,
    frontmatter: null,
    blockCount: 0,
    ...over,
  };
}

function makeDeps(): CallbackDeps & {
  dispatchDocReplace: ReturnType<typeof vi.fn>;
  returnFocusToBody: ReturnType<typeof vi.fn>;
} {
  return {
    dispatchDocReplace: vi.fn(),
    returnFocusToBody: vi.fn(),
  };
}

describe("buildDocHeaderCallbacks", () => {
  describe("when entry is undefined", () => {
    it("editable callbacks are no-ops (no view to dispatch into)", () => {
      const deps = makeDeps();
      const cb = buildDocHeaderCallbacks(undefined, "i1", deps);
      cb.onTitleSave("New title");
      cb.onAbstractSave("Some abstract");
      cb.onAddTag("foo");
      cb.onRemoveTag("foo");
      cb.onChecklistSave([]);
      expect(deps.dispatchDocReplace).not.toHaveBeenCalled();
    });

    it("onTitleNavigateToBody still routes to returnFocusToBody (V6 cenário 3)", () => {
      const deps = makeDeps();
      const cb = buildDocHeaderCallbacks(undefined, "i1", deps);
      cb.onTitleNavigateToBody();
      expect(deps.returnFocusToBody).toHaveBeenCalledWith("i1");
    });
  });

  describe("when entry has no view bound", () => {
    it("every editable callback bails", () => {
      const deps = makeDeps();
      const entry = makeEntry({ view: null });
      const cb = buildDocHeaderCallbacks(entry, "i1", deps);
      cb.onTitleSave("x");
      cb.onAbstractSave("x");
      cb.onAddTag("x");
      cb.onRemoveTag("x");
      cb.onChecklistSave([]);
      expect(deps.dispatchDocReplace).not.toHaveBeenCalled();
    });
  });

  describe("with a view bound", () => {
    it("onTitleSave dispatches a frontmatter rewrite", () => {
      const deps = makeDeps();
      const view = makeFakeView("---\ntitle: Old\n---\nbody\n");
      const entry = makeEntry({ view });
      const cb = buildDocHeaderCallbacks(entry, "i1", deps);
      cb.onTitleSave("New");
      expect(deps.dispatchDocReplace).toHaveBeenCalledTimes(1);
      const [calledView, content] = deps.dispatchDocReplace.mock.calls[0]!;
      expect(calledView).toBe(view);
      expect(content).toContain("New");
    });

    it("onAbstractSave dispatches a frontmatter rewrite", () => {
      const deps = makeDeps();
      const view = makeFakeView("---\nabstract: Old\n---\n");
      const entry = makeEntry({ view });
      const cb = buildDocHeaderCallbacks(entry, "i1", deps);
      cb.onAbstractSave("Better");
      expect(deps.dispatchDocReplace).toHaveBeenCalledTimes(1);
      expect(deps.dispatchDocReplace.mock.calls[0]![1]).toContain("Better");
    });

    describe("onAddTag", () => {
      it("appends a new tag to the existing list", () => {
        const deps = makeDeps();
        const view = makeFakeView("---\ntags: [a]\n---\n");
        const entry = makeEntry({
          view,
          frontmatter: { tags: ["a"] },
        });
        const cb = buildDocHeaderCallbacks(entry, "i1", deps);
        cb.onAddTag("b");
        const next = deps.dispatchDocReplace.mock.calls[0]![1];
        expect(next).toContain("a");
        expect(next).toContain("b");
      });

      it("trims whitespace before adding", () => {
        const deps = makeDeps();
        const view = makeFakeView("---\ntags: []\n---\n");
        const entry = makeEntry({
          view,
          frontmatter: { tags: [] },
        });
        const cb = buildDocHeaderCallbacks(entry, "i1", deps);
        cb.onAddTag("  spaced  ");
        const next = deps.dispatchDocReplace.mock.calls[0]![1];
        expect(next).toContain("spaced");
        expect(next).not.toContain("  spaced  ");
      });

      it("ignores empty / whitespace-only tags", () => {
        const deps = makeDeps();
        const view = makeFakeView("---\ntags: []\n---\n");
        const entry = makeEntry({ view, frontmatter: { tags: [] } });
        const cb = buildDocHeaderCallbacks(entry, "i1", deps);
        cb.onAddTag("   ");
        cb.onAddTag("");
        expect(deps.dispatchDocReplace).not.toHaveBeenCalled();
      });

      it("dedups: skips tags already present", () => {
        const deps = makeDeps();
        const view = makeFakeView("---\ntags: [a]\n---\n");
        const entry = makeEntry({
          view,
          frontmatter: { tags: ["a"] },
        });
        const cb = buildDocHeaderCallbacks(entry, "i1", deps);
        cb.onAddTag("a");
        expect(deps.dispatchDocReplace).not.toHaveBeenCalled();
      });

      it("works when frontmatter has no tags yet", () => {
        const deps = makeDeps();
        const view = makeFakeView("---\ntitle: x\n---\n");
        const entry = makeEntry({ view, frontmatter: { title: "x" } });
        const cb = buildDocHeaderCallbacks(entry, "i1", deps);
        cb.onAddTag("first");
        expect(deps.dispatchDocReplace).toHaveBeenCalledTimes(1);
      });
    });

    describe("onRemoveTag", () => {
      it("removes a tag and rewrites the list", () => {
        const deps = makeDeps();
        const view = makeFakeView("---\ntags: [a, b]\n---\n");
        const entry = makeEntry({
          view,
          frontmatter: { tags: ["a", "b"] },
        });
        const cb = buildDocHeaderCallbacks(entry, "i1", deps);
        cb.onRemoveTag("a");
        const next = deps.dispatchDocReplace.mock.calls[0]![1];
        expect(next).toContain("b");
        expect(next).not.toMatch(/\ba\b/);
      });

      it("is a no-op when the tag isn't in the list", () => {
        const deps = makeDeps();
        const view = makeFakeView("---\ntags: [a]\n---\n");
        const entry = makeEntry({
          view,
          frontmatter: { tags: ["a"] },
        });
        const cb = buildDocHeaderCallbacks(entry, "i1", deps);
        cb.onRemoveTag("missing");
        expect(deps.dispatchDocReplace).not.toHaveBeenCalled();
      });

      it("is a no-op when frontmatter has no tags", () => {
        const deps = makeDeps();
        const view = makeFakeView("---\ntitle: x\n---\n");
        const entry = makeEntry({ view, frontmatter: { title: "x" } });
        const cb = buildDocHeaderCallbacks(entry, "i1", deps);
        cb.onRemoveTag("a");
        expect(deps.dispatchDocReplace).not.toHaveBeenCalled();
      });
    });

    it("onChecklistSave rewrites the tasks list", () => {
      const deps = makeDeps();
      const view = makeFakeView("---\ntasks: [\"[ ] one\"]\n---\n");
      const entry = makeEntry({ view });
      const cb = buildDocHeaderCallbacks(entry, "i1", deps);
      cb.onChecklistSave([
        { text: "one", done: true },
        { text: "two", done: false },
      ]);
      expect(deps.dispatchDocReplace).toHaveBeenCalledTimes(1);
      const next = deps.dispatchDocReplace.mock.calls[0]![1];
      expect(next).toContain("[x] one");
      expect(next).toContain("[ ] two");
    });
  });

  it("onTitleNavigateToBody passes the bound instanceId (V6 cenário 3)", () => {
    const deps = makeDeps();
    const cb = buildDocHeaderCallbacks(makeEntry(), "instance-42", deps);
    cb.onTitleNavigateToBody();
    expect(deps.returnFocusToBody).toHaveBeenCalledWith("instance-42");
  });
});
