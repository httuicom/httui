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
