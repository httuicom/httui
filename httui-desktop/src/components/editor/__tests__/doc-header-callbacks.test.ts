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

  describe("preflight checks (V6 cenário 9)", () => {
    it("onAddPreflightCheck appends to the block-list", () => {
      const deps = makeDeps();
      const view = makeFakeView(
        "---\npreflight:\n  - connection: a\n---\nbody\n",
      );
      const entry = makeEntry({ view });
      const cb = buildDocHeaderCallbacks(entry, "i1", deps);
      cb.onAddPreflightCheck({ kind: "command", value: "psql" });
      const next = deps.dispatchDocReplace.mock.calls[0]![1];
      expect(next).toContain("- connection: a");
      expect(next).toContain("- command: psql");
    });

    it("onAddPreflightCheck creates a new block when none exists", () => {
      const deps = makeDeps();
      const view = makeFakeView("---\ntitle: x\n---\nbody\n");
      const entry = makeEntry({ view });
      const cb = buildDocHeaderCallbacks(entry, "i1", deps);
      cb.onAddPreflightCheck({ kind: "env_var", value: "API_TOKEN" });
      const next = deps.dispatchDocReplace.mock.calls[0]![1];
      expect(next).toContain("preflight:");
      expect(next).toContain("- env_var: API_TOKEN");
    });

    it("onEditPreflightCheck replaces the check at idx", () => {
      const deps = makeDeps();
      const view = makeFakeView(
        "---\npreflight:\n  - connection: old\n  - command: ls\n---\n",
      );
      const entry = makeEntry({ view });
      const cb = buildDocHeaderCallbacks(entry, "i1", deps);
      cb.onEditPreflightCheck(0, { kind: "connection", value: "new" });
      const next = deps.dispatchDocReplace.mock.calls[0]![1];
      expect(next).toContain("- connection: new");
      expect(next).toContain("- command: ls");
      expect(next).not.toContain("- connection: old");
    });

    it("onEditPreflightCheck is a no-op for out-of-range idx", () => {
      const deps = makeDeps();
      const view = makeFakeView("---\npreflight:\n  - command: ls\n---\n");
      const entry = makeEntry({ view });
      const cb = buildDocHeaderCallbacks(entry, "i1", deps);
      cb.onEditPreflightCheck(5, { kind: "command", value: "x" });
      expect(deps.dispatchDocReplace).not.toHaveBeenCalled();
    });

    it("onRemovePreflightCheck drops the check at idx", () => {
      const deps = makeDeps();
      const view = makeFakeView(
        "---\npreflight:\n  - connection: a\n  - command: ls\n---\n",
      );
      const entry = makeEntry({ view });
      const cb = buildDocHeaderCallbacks(entry, "i1", deps);
      cb.onRemovePreflightCheck(0);
      const next = deps.dispatchDocReplace.mock.calls[0]![1];
      expect(next).toContain("- command: ls");
      expect(next).not.toContain("- connection: a");
    });

    it("onRemovePreflightCheck on the last item drops the block entirely", () => {
      const deps = makeDeps();
      const view = makeFakeView("---\npreflight:\n  - command: ls\n---\n");
      const entry = makeEntry({ view });
      const cb = buildDocHeaderCallbacks(entry, "i1", deps);
      cb.onRemovePreflightCheck(0);
      const next = deps.dispatchDocReplace.mock.calls[0]![1];
      expect(next).not.toContain("preflight:");
    });

    it("preflight callbacks are no-ops when entry has no view", () => {
      const deps = makeDeps();
      const entry = makeEntry({ view: null });
      const cb = buildDocHeaderCallbacks(entry, "i1", deps);
      cb.onAddPreflightCheck({ kind: "command", value: "ls" });
      cb.onEditPreflightCheck(0, { kind: "command", value: "x" });
      cb.onRemovePreflightCheck(0);
      expect(deps.dispatchDocReplace).not.toHaveBeenCalled();
    });
  });
});
