import { afterEach, describe, expect, it, vi } from "vitest";
import { EditorSelection, EditorState, Text } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

import {
  createDocHeaderExtension,
  dispatchDocReplace,
  findFrontmatterRange,
  getDocHeaderEntries,
  registerDocHeaderTitleInput,
  returnFocusToBody,
} from "@/lib/codemirror/cm-doc-header";

function asDoc(text: string) {
  return Text.of(text.split("\n"));
}

function createView(doc: string, instanceId?: { current: string }) {
  const handle = createDocHeaderExtension();
  if (instanceId) instanceId.current = handle.instanceId;
  const state = EditorState.create({
    doc,
    extensions: [handle.extension],
  });
  return new EditorView({ state, parent: document.body });
}

describe("findFrontmatterRange", () => {
  it("returns null for an empty doc", () => {
    expect(findFrontmatterRange(asDoc(""))).toBeNull();
  });

  it("returns null when the doc has no opening fence", () => {
    expect(findFrontmatterRange(asDoc("# Heading\n\nbody"))).toBeNull();
  });

  it("returns null when the opening fence is not on line 1", () => {
    expect(findFrontmatterRange(asDoc("\n---\ntitle: x\n---\n"))).toBeNull();
  });

  it("returns null when the opening fence has no matching close", () => {
    expect(
      findFrontmatterRange(asDoc("---\ntitle: x\nbody body body")),
    ).toBeNull();
  });

  it("detects a simple single-key frontmatter", () => {
    const doc = asDoc("---\ntitle: Hello\n---\nbody");
    const range = findFrontmatterRange(doc);
    // 0..3 = "---", 4..16 = "title: Hello\n", 17..19 = "---"
    // After the closing `---` line.to == 20 (end of line 3 inclusive of \n).
    // We swallow the trailing newline so body cursor lands on offset 20.
    expect(range).not.toBeNull();
    expect(range!.from).toBe(0);
    // Range covers `---\ntitle: Hello\n---\n` = 21 chars.
    expect(range!.to).toBe(21);
  });

  it("detects a multi-line frontmatter", () => {
    const doc = asDoc(
      "---\ntitle: Hello\nabstract: World\ntags: [a, b]\n---\nbody",
    );
    const range = findFrontmatterRange(doc);
    expect(range).not.toBeNull();
    expect(range!.from).toBe(0);
    // Length of frontmatter incl. trailing \n: 51 ?
    // Let's compute: "---\n" = 4, "title: Hello\n" = 13, "abstract: World\n" = 16,
    // "tags: [a, b]\n" = 13, "---\n" = 4. Total = 50.
    expect(range!.to).toBe(50);
  });

  it("does not confuse a `---` separator in the middle of the body", () => {
    const doc = asDoc("# Heading\n\n---\n\nbelow the rule");
    expect(findFrontmatterRange(doc)).toBeNull();
  });

  it("requires the close fence to be exactly `---`", () => {
    // `--- ` with trailing space is not a fence terminator (must be exact).
    const doc = asDoc("---\ntitle: x\n--- \nbody");
    expect(findFrontmatterRange(doc)).toBeNull();
  });

  it("handles frontmatter that occupies the entire doc (no body)", () => {
    const doc = asDoc("---\ntitle: x\n---");
    const range = findFrontmatterRange(doc);
    expect(range).not.toBeNull();
    expect(range!.from).toBe(0);
    // No trailing newline to swallow — `to` is doc length.
    expect(range!.to).toBe(doc.length);
  });

  it("handles an empty frontmatter body", () => {
    const doc = asDoc("---\n---\n# body");
    const range = findFrontmatterRange(doc);
    expect(range).not.toBeNull();
    // "---\n---\n" = 8 chars.
    expect(range!.to).toBe(8);
  });
});

describe("DocHeader nav keymap (M3)", () => {
  afterEach(() => {
    document.body.innerHTML = "";
  });

  it("focuses the title input on ArrowUp at body start", () => {
    const idRef = { current: "" };
    const view = createView("---\ntitle: x\n---\nbody line one\n", idRef);

    const fakeInput = document.createElement("input");
    fakeInput.tabIndex = 0;
    document.body.appendChild(fakeInput);
    registerDocHeaderTitleInput(idRef.current, fakeInput);
    const focusSpy = vi.spyOn(fakeInput, "focus");

    // Cursor on body's first line (right after the frontmatter range).
    const bodyStart = getDocHeaderEntries().get(idRef.current)?.lastBodyOffset;
    expect(typeof bodyStart).toBe("number");
    view.dispatch({ selection: EditorSelection.cursor(13) });
    // 13 = "---\ntitle: x\n---\n" length minus 1? Let's compute: "---\n" = 4
    // + "title: x\n" = 9 + "---\n" = 4 → 17. Body starts at 17. The
    // ArrowUp run() takes a doc.lineAt(head) check, so any offset on
    // line 4 (the body's first line) qualifies. Let's set head to 17.
    view.dispatch({ selection: EditorSelection.cursor(17) });

    // Synthetic keydown.
    const ev = new KeyboardEvent("keydown", {
      key: "ArrowUp",
      bubbles: true,
      cancelable: true,
    });
    view.contentDOM.dispatchEvent(ev);

    expect(focusSpy).toHaveBeenCalled();
    view.destroy();
  });

  it("does NOT focus the input when cursor is below the body's first line", () => {
    const idRef = { current: "" };
    const view = createView(
      "---\ntitle: x\n---\nline one\nline two\n",
      idRef,
    );

    const fakeInput = document.createElement("input");
    document.body.appendChild(fakeInput);
    registerDocHeaderTitleInput(idRef.current, fakeInput);
    const focusSpy = vi.spyOn(fakeInput, "focus");

    // Body starts at offset 17; "line one\n" runs 17..26, so 27 lands
    // on "line two".
    view.dispatch({ selection: EditorSelection.cursor(27) });
    view.contentDOM.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowUp", bubbles: true }),
    );
    expect(focusSpy).not.toHaveBeenCalled();
    view.destroy();
  });

  it("returnFocusToBody dispatches a selection to the last body offset", () => {
    const idRef = { current: "" };
    const view = createView(
      "---\ntitle: x\n---\nline one\nline two\nline three\n",
      idRef,
    );

    // Move cursor deep into the body — `lastBodyOffset` should track.
    const lastOffset = 30;
    view.dispatch({ selection: EditorSelection.cursor(lastOffset) });

    const fakeInput = document.createElement("input");
    document.body.appendChild(fakeInput);
    registerDocHeaderTitleInput(idRef.current, fakeInput);
    fakeInput.focus();
    expect(document.activeElement).toBe(fakeInput);

    returnFocusToBody(idRef.current);

    expect(view.state.selection.main.head).toBe(lastOffset);
    view.destroy();
  });

  it("returnFocusToBody clamps to body start when last offset is missing", () => {
    const idRef = { current: "" };
    const view = createView("---\ntitle: x\n---\nbody\n", idRef);
    // No prior body cursor placement → entry.lastBodyOffset defaults
    // to 0 (inside the frontmatter range), so the helper falls back
    // to `range.to` (the body start).
    const entry = getDocHeaderEntries().get(idRef.current);
    expect(entry).toBeDefined();
    entry!.lastBodyOffset = 0;
    returnFocusToBody(idRef.current);
    // Body starts at offset 17.
    expect(view.state.selection.main.head).toBe(17);
    view.destroy();
  });

  it("ignores ArrowUp when no titleInput is registered", () => {
    const idRef = { current: "" };
    const view = createView("---\ntitle: x\n---\nbody line\n", idRef);
    // Skip the registerDocHeaderTitleInput call.
    view.dispatch({ selection: EditorSelection.cursor(17) });

    // The default ArrowUp behavior is "stay put" at line 1 since
    // there's no line above; we just check no exception throws.
    expect(() => {
      view.contentDOM.dispatchEvent(
        new KeyboardEvent("keydown", { key: "ArrowUp", bubbles: true }),
      );
    }).not.toThrow();
    view.destroy();
  });
});

describe("DocHeader frontmatter guard (transactionFilter)", () => {
  afterEach(() => {
    document.body.innerHTML = "";
  });

  it("blocks a Backspace that would delete the frontmatter close fence", () => {
    const view = createView("---\ntitle: x\n---\nbody\n");
    const before = view.state.doc.toString();
    // Cursor at body start (offset 17). Default Backspace removes the
    // char at 16 (the trailing \n of the closing fence).
    view.dispatch({ selection: EditorSelection.cursor(17) });
    view.dispatch({ changes: { from: 16, to: 17, insert: "" } });
    expect(view.state.doc.toString()).toBe(before);
    view.destroy();
  });

  it("blocks a range deletion that crosses the frontmatter boundary", () => {
    const view = createView("---\ntitle: x\n---\nbody\n");
    const before = view.state.doc.toString();
    // Selection from inside the frontmatter (5) to inside the body
    // (20) — full delete would chew through the YAML.
    view.dispatch({ changes: { from: 5, to: 20, insert: "" } });
    expect(view.state.doc.toString()).toBe(before);
    view.destroy();
  });

  it("allows backspace inside the body (no frontmatter overlap)", () => {
    const view = createView("---\ntitle: x\n---\nbody\n");
    // Body chars: b(17) o(18) d(19) y(20). Delete the 'y' at 20.
    view.dispatch({ changes: { from: 20, to: 21, insert: "" } });
    expect(view.state.doc.toString()).toBe("---\ntitle: x\n---\nbod\n");
    view.destroy();
  });

  it("allows programmatic frontmatter rewrites via dispatchDocReplace", () => {
    const view = createView("---\ntitle: Old\n---\nbody\n");
    dispatchDocReplace(view, "---\ntitle: New\n---\nbody\n");
    expect(view.state.doc.toString()).toBe("---\ntitle: New\n---\nbody\n");
    view.destroy();
  });

  it("allows changes when the doc has no frontmatter", () => {
    const view = createView("# heading\nbody\n");
    view.dispatch({ changes: { from: 0, to: 1, insert: "" } });
    expect(view.state.doc.toString()).toBe(" heading\nbody\n");
    view.destroy();
  });

  // Cmd-A keymap binding can't be exercised in jsdom — synthetic
  // KeyboardEvents don't route through CM6's keymap dispatcher
  // reliably for Mod-prefixed combos. The behavior is covered by
  // the static cases below: the user's "select all then delete"
  // bug class is the doc-spanning change being blocked. We verify
  // BOTH paths: unclipped (full doc range) gets blocked, clipped
  // (body-only range, what the keymap produces) goes through.
  it("blocks a doc-spanning delete when Cmd-A's clip didn't run", () => {
    const view = createView("---\ntitle: x\n---\nbody one\nbody two\n");
    const before = view.state.doc.toString();
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: "" },
    });
    // Guard rejects — frontmatter range overlaps.
    expect(view.state.doc.toString()).toBe(before);
    view.destroy();
  });

  it("body-clipped Cmd-A delete erases only the body", () => {
    const view = createView("---\ntitle: x\n---\nbody one\nbody two\n");
    // Body starts at offset 17. The Cmd-A keymap clips selection to
    // [17, doc.length]; deleting that range hits no frontmatter
    // bytes so the guard lets it pass.
    const bodyStart = 17;
    view.dispatch({
      changes: { from: bodyStart, to: view.state.doc.length, insert: "" },
    });
    expect(view.state.doc.toString()).toBe("---\ntitle: x\n---\n");
    view.destroy();
  });
});
