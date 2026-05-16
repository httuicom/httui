import { describe, it, expect, afterEach } from "vitest";
import { Text, EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

import {
  parseConflictRegions,
  mergeConflict,
} from "@/lib/codemirror/cm-merge-conflict";

const lines = (s: string) => Text.of(s.split("\n"));

const CONFLICT = [
  "intro",
  "<<<<<<< HEAD",
  "ours line",
  "=======",
  "theirs line",
  ">>>>>>> branch",
  "tail",
].join("\n");

describe("parseConflictRegions", () => {
  it("finds a well-formed hunk", () => {
    const r = parseConflictRegions(lines(CONFLICT));
    expect(r).toEqual([
      { oursMarker: 2, separator: 4, theirsMarker: 6 },
    ]);
  });

  it("returns nothing for a clean doc", () => {
    expect(parseConflictRegions(lines("# title\n\nbody"))).toEqual([]);
  });

  it("ignores a marker run with no separator", () => {
    const doc = lines("<<<<<<< HEAD\nx\n>>>>>>> b");
    expect(parseConflictRegions(doc)).toEqual([]);
  });

  it("handles multiple hunks", () => {
    const doc = lines(
      [
        "<<<<<<< HEAD",
        "a",
        "=======",
        "b",
        ">>>>>>> x",
        "mid",
        "<<<<<<< HEAD",
        "c",
        "=======",
        "d",
        ">>>>>>> y",
      ].join("\n"),
    );
    expect(parseConflictRegions(doc)).toHaveLength(2);
  });
});

describe("mergeConflict extension", () => {
  let view: EditorView;

  afterEach(() => view?.destroy());

  function mount(doc: string) {
    view = new EditorView({
      state: EditorState.create({ doc, extensions: [mergeConflict()] }),
      parent: document.body,
    });
    return view;
  }

  it("decorates ours/theirs/marker lines + mounts the toolbar", () => {
    mount(CONFLICT);
    expect(view.dom.querySelector(".cm-conflict-ours")).toBeTruthy();
    expect(view.dom.querySelector(".cm-conflict-theirs")).toBeTruthy();
    expect(view.dom.querySelector(".cm-conflict-sep")).toBeTruthy();
    expect(
      view.dom.querySelectorAll(".cm-conflict-marker"),
    ).toHaveLength(2);
    expect(view.dom.querySelector(".cm-conflict-toolbar")).toBeTruthy();
  });

  it("Accept current keeps only the ours side", () => {
    mount(CONFLICT);
    const btn = view.dom.querySelector(
      '.cm-conflict-btn[data-side="ours"]',
    ) as HTMLButtonElement;
    btn.dispatchEvent(
      new MouseEvent("mousedown", { bubbles: true, cancelable: true }),
    );
    const out = view.state.doc.toString();
    expect(out).toContain("ours line");
    expect(out).not.toContain("theirs line");
    expect(out).not.toContain("<<<<<<<");
  });

  it("Accept incoming keeps only the theirs side", () => {
    mount(CONFLICT);
    (
      view.dom.querySelector(
        '.cm-conflict-btn[data-side="theirs"]',
      ) as HTMLButtonElement
    ).dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
    const out = view.state.doc.toString();
    expect(out).toContain("theirs line");
    expect(out).not.toContain("ours line");
    expect(out).not.toContain(">>>>>>>");
  });

  it("Accept both keeps ours then theirs without markers", () => {
    mount(CONFLICT);
    (
      view.dom.querySelector(
        '.cm-conflict-btn[data-side="both"]',
      ) as HTMLButtonElement
    ).dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
    const out = view.state.doc.toString();
    expect(out).toContain("ours line\ntheirs line");
    expect(out).not.toMatch(/[<=>]{7}/);
  });

  it("clears decorations once the conflict is resolved", () => {
    mount(CONFLICT);
    (
      view.dom.querySelector(
        '.cm-conflict-btn[data-side="ours"]',
      ) as HTMLButtonElement
    ).dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
    expect(view.dom.querySelector(".cm-conflict-toolbar")).toBeNull();
  });
});
