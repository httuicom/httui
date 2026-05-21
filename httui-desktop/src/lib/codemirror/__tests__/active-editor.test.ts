import { afterEach, describe, expect, it } from "vitest";
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

import {
  activeEditorTracker,
  getActiveEditor,
  registerActiveEditor,
  unregisterActiveEditor,
} from "@/lib/codemirror/active-editor";

const views: EditorView[] = [];

function makeView(withTracker = true): EditorView {
  const view = new EditorView({
    state: EditorState.create({
      doc: "hello",
      extensions: withTracker ? [activeEditorTracker()] : [],
    }),
    parent: document.body,
  });
  views.push(view);
  return view;
}

function focusin(view: EditorView) {
  view.contentDOM.dispatchEvent(new Event("focusin", { bubbles: true }));
}
function focusout(view: EditorView) {
  view.contentDOM.dispatchEvent(new Event("focusout", { bubbles: true }));
}

afterEach(() => {
  while (views.length) views.pop()!.destroy();
  // Ensure no view leaks into the next test's module state.
  const active = getActiveEditor();
  if (active) unregisterActiveEditor(active);
});

describe("active-editor registry", () => {
  it("register / get / unregister are scoped to the same view", () => {
    const a = makeView(false);
    const b = makeView(false);

    registerActiveEditor(a);
    expect(getActiveEditor()).toBe(a);

    // Unregistering a non-active view is a no-op.
    unregisterActiveEditor(b);
    expect(getActiveEditor()).toBe(a);

    unregisterActiveEditor(a);
    expect(getActiveEditor()).toBeNull();
  });
});

describe("activeEditorTracker", () => {
  it("registers on focusin and clears on focusout", () => {
    const view = makeView();
    expect(getActiveEditor()).toBeNull();

    focusin(view);
    expect(getActiveEditor()).toBe(view);

    focusout(view);
    expect(getActiveEditor()).toBeNull();
  });

  it("unregisters when the view is destroyed", () => {
    const view = makeView();
    focusin(view);
    expect(getActiveEditor()).toBe(view);

    view.destroy();
    expect(getActiveEditor()).toBeNull();
  });

  it("does not leak listeners: a destroyed view never re-registers", () => {
    const view = makeView();
    focusin(view);
    expect(getActiveEditor()).toBe(view);

    const dom = view.contentDOM;
    view.destroy();
    expect(getActiveEditor()).toBeNull();

    // The old DOM is detached; CM removed its handlers on destroy. A
    // stray focus event must NOT resurrect the dead view (the bug this
    // fix addresses: shell-side listeners were never removed).
    dom.dispatchEvent(new Event("focusin", { bubbles: true }));
    expect(getActiveEditor()).toBeNull();
  });

  it("destroying a non-active view does not clobber the active one", () => {
    const a = makeView();
    const b = makeView();

    focusin(a);
    focusin(b);
    expect(getActiveEditor()).toBe(b);

    a.destroy();
    expect(getActiveEditor()).toBe(b);
  });
});

describe("insertDbSnippetIntoActiveEditor", () => {
  // Imports placed inline so the suite stays self-contained — the
  // top-level imports cover the registry/tracker surface.
  it("returns false when no active editor is registered", async () => {
    const { insertDbSnippetIntoActiveEditor } =
      await import("@/lib/codemirror/active-editor");
    const out = insertDbSnippetIntoActiveEditor({
      snippet: "SELECT 1",
      dialect: "postgres",
    });
    expect(out).toBe(false);
  });

  it("replaces the body of the db block the cursor is inside", async () => {
    const { insertDbSnippetIntoActiveEditor } =
      await import("@/lib/codemirror/active-editor");
    const view = new EditorView({
      state: EditorState.create({
        doc: "```db-postgres alias=q\nSELECT 1\n```",
        // Position cursor in the body (line 2).
        selection: { anchor: 30 },
      }),
      parent: document.body,
    });
    views.push(view);
    registerActiveEditor(view);
    const ok = insertDbSnippetIntoActiveEditor({
      snippet: "SELECT 2",
      dialect: "postgres",
    });
    expect(ok).toBe(true);
    expect(view.state.doc.toString()).toContain("SELECT 2");
    // The original SELECT 1 should be gone.
    expect(view.state.doc.toString()).not.toContain("SELECT 1");
  });

  it("inserts a new db fenced block when cursor is outside any block", async () => {
    const { insertDbSnippetIntoActiveEditor } =
      await import("@/lib/codemirror/active-editor");
    const view = new EditorView({
      state: EditorState.create({
        doc: "Some prose text",
        // Cursor at end of the prose line — non-empty + not at line start.
        selection: { anchor: 15 },
      }),
      parent: document.body,
    });
    views.push(view);
    registerActiveEditor(view);
    const ok = insertDbSnippetIntoActiveEditor({
      snippet: "SELECT 1",
      dialect: "postgres",
      alias: "myq",
      connection: "prod",
    });
    expect(ok).toBe(true);
    const text = view.state.doc.toString();
    // Leading newline since line has content + not at line start.
    expect(text).toContain("\n```db-postgres");
    expect(text).toContain("alias=myq");
    expect(text).toContain("connection=prod");
    expect(text).toContain("SELECT 1");
    expect(text).toMatch(/```\n$/);
  });

  it("omits leading newline when the cursor is at the start of a line", async () => {
    const { insertDbSnippetIntoActiveEditor } =
      await import("@/lib/codemirror/active-editor");
    const view = new EditorView({
      state: EditorState.create({
        doc: "First line\n",
        // Cursor at start of empty second line.
        selection: { anchor: 11 },
      }),
      parent: document.body,
    });
    views.push(view);
    registerActiveEditor(view);
    insertDbSnippetIntoActiveEditor({
      snippet: "SELECT 1",
      dialect: "sqlite",
    });
    // The insertion at pos=11 should NOT prepend a newline.
    const text = view.state.doc.toString();
    // Char at position 11 (right after the first \n) is the opening backtick.
    expect(text.slice(11, 14)).toBe("```");
  });

  it("uses default alias 'db1' when alias not supplied", async () => {
    const { insertDbSnippetIntoActiveEditor } =
      await import("@/lib/codemirror/active-editor");
    const view = new EditorView({
      state: EditorState.create({ doc: "" }),
      parent: document.body,
    });
    views.push(view);
    registerActiveEditor(view);
    insertDbSnippetIntoActiveEditor({
      snippet: "SELECT 1",
      dialect: "mysql",
    });
    expect(view.state.doc.toString()).toContain("alias=db1");
  });
});
