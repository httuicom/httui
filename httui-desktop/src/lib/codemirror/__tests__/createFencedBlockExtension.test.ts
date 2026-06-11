/**
 * Direct tests of `createFencedBlockExtension` factory + helpers.
 * The cm-http-block.test / cm-db-block.test integration tests
 * exercise the happy paths through the full extension; these tests
 * cover the generic factory's edge cases (no match, empty body,
 * keymap require/loose semantics, extraExtensions).
 */
import { describe, it, expect, vi } from "vitest";
import {
  EditorState,
  EditorSelection,
  StateField,
  Text,
} from "@codemirror/state";
import { Decoration, EditorView } from "@codemirror/view";

import {
  FENCE_CLOSE_RE,
  blockAtCursor,
  createFencedBlockExtension,
  cursorInsideBlock,
  makeFencedKeymap,
  makeFencedScanner,
} from "@/lib/codemirror/createFencedBlockExtension";
import {
  WidgetPortalRegistry,
  type FencedBlockBase,
} from "@/lib/codemirror/widget-portal-registry";

interface FakeBlock extends FencedBlockBase {
  metadata: { alias?: string };
  lang?: string;
}

const openRe = /^```fake(.*)$/;

const scanner = makeFencedScanner<FakeBlock, Partial<FakeBlock>>({
  openRe,
  parse: (match) => ({
    info: match[1].trim(),
    metadata: { alias: match[1].trim() || undefined },
  }),
});

function mkRegistry() {
  return new WidgetPortalRegistry<
    "toolbar" | "result",
    { onRun?: () => void; onCancel?: () => void },
    FakeBlock
  >({
    idPrefix: "fake_idx_",
    slots: ["toolbar", "result"],
    metaChanged: (a, b) => a.metadata.alias !== b.metadata.alias,
    bodyChangePolicy: "immediate",
    dedupeSameSlotElement: false,
  });
}

describe("FENCE_CLOSE_RE", () => {
  it("matches 3+ backticks optionally followed by whitespace", () => {
    expect(FENCE_CLOSE_RE.test("```")).toBe(true);
    expect(FENCE_CLOSE_RE.test("````")).toBe(true);
    expect(FENCE_CLOSE_RE.test("``` ")).toBe(true);
    expect(FENCE_CLOSE_RE.test("``")).toBe(false);
    expect(FENCE_CLOSE_RE.test("```fake")).toBe(false);
  });
});

describe("makeFencedScanner — findBlocks", () => {
  it("returns [] when no opening fence is present", () => {
    expect(scanner.findBlocks(Text.of(["# heading", "plain"]))).toEqual([]);
  });

  it("ignores blocks without a closing fence", () => {
    expect(
      scanner.findBlocks(Text.of(["```fake alias=a", "body", "(no close)"])),
    ).toEqual([]);
  });

  it("collapses the bodyTo to bodyFrom for an empty body", () => {
    const blocks = scanner.findBlocks(Text.of(["```fake", "```"]));
    expect(blocks).toHaveLength(1);
    expect(blocks[0].body).toBe("");
    expect(blocks[0].bodyFrom).toBe(blocks[0].bodyTo);
  });

  it("preserves multi-line body verbatim incl. blank lines", () => {
    const blocks = scanner.findBlocks(
      Text.of(["```fake", "a", "", "b", "```"]),
    );
    expect(blocks).toHaveLength(1);
    expect(blocks[0].body).toBe("a\n\nb");
  });

  it("captures metadata from the info string", () => {
    const blocks = scanner.findBlocks(
      Text.of(["```fake alias=req", "x", "```"]),
    );
    expect(blocks[0].metadata.alias).toBe("alias=req");
  });
});

describe("makeFencedScanner — countBlocks", () => {
  it("counts opening fences only (not the close)", () => {
    const doc = Text.of([
      "```fake a",
      "x",
      "```",
      "",
      "```fake b",
      "y",
      "```",
      "",
      "```other",
      "ignore",
      "```",
    ]);
    expect(scanner.countBlocks(doc)).toBe(2);
  });

  it("returns 0 for a doc with no opening fences", () => {
    expect(scanner.countBlocks(Text.of(["plain text"]))).toBe(0);
  });
});

describe("cursorInsideBlock", () => {
  function mkBlock(from: number, to: number): FakeBlock {
    return {
      from,
      to,
      info: "",
      openLineFrom: from,
      openLineTo: from + 2,
      bodyFrom: from + 3,
      bodyTo: to - 3,
      closeLineFrom: to - 2,
      closeLineTo: to,
      body: "",
      metadata: {},
    };
  }
  function mkState(doc: string, cursor: number) {
    return EditorState.create({
      doc,
      selection: EditorSelection.single(cursor),
    });
  }

  it("true when cursor is between block.from and block.to", () => {
    const state = mkState("xxxxxxxxxxxxxxxxxxxxxxxxx", 8);
    expect(cursorInsideBlock(state, mkBlock(5, 15))).toBe(true);
  });

  it("true on the boundary positions (inclusive)", () => {
    const state = mkState("xxxxxxxxxxxxxxxxxxxxxxxxx", 5);
    expect(cursorInsideBlock(state, mkBlock(5, 15))).toBe(true);
  });

  it("false when cursor is outside", () => {
    const state = mkState("xxxxxxxxxxxxxxxxxxxxxxxxx", 20);
    expect(cursorInsideBlock(state, mkBlock(5, 15))).toBe(false);
  });
});

describe("blockAtCursor", () => {
  it("finds the block whose range contains the cursor + returns the registry entry", () => {
    const r = mkRegistry();
    const doc = "```fake a\nbody\n```";
    const blocks = scanner.findBlocks(Text.of(doc.split("\n")));
    r.registerSlot(
      r.blockIdOf(blocks[0], 0),
      blocks[0],
      "toolbar",
      document.createElement("div"),
    );
    const state = EditorState.create({
      doc,
      selection: EditorSelection.single(5),
    });
    const view = new EditorView({ state, parent: document.body });
    try {
      const found = blockAtCursor(view, blocks, r);
      expect(found).not.toBeNull();
      expect(found!.block).toBe(blocks[0]);
    } finally {
      view.destroy();
    }
  });

  it("returns null when no block matches", () => {
    const r = mkRegistry();
    const view = new EditorView({
      state: EditorState.create({ doc: "plain" }),
      parent: document.body,
    });
    try {
      expect(blockAtCursor(view, [], r)).toBeNull();
    } finally {
      view.destroy();
    }
  });

  it("returns null when the block is in range but has no registry entry yet", () => {
    const r = mkRegistry();
    const doc = "```fake\nx\n```";
    const blocks = scanner.findBlocks(Text.of(doc.split("\n")));
    // NO registerSlot call → entry doesn't exist.
    const state = EditorState.create({
      doc,
      selection: EditorSelection.single(5),
    });
    const view = new EditorView({ state, parent: document.body });
    try {
      expect(blockAtCursor(view, blocks, r)).toBeNull();
    } finally {
      view.destroy();
    }
  });
});

describe("makeFencedKeymap", () => {
  function setup() {
    const r = mkRegistry();
    const doc = "```fake a\nbody\n```";
    const blocks = scanner.findBlocks(Text.of(doc.split("\n")));
    r.registerSlot(
      r.blockIdOf(blocks[0], 0),
      blocks[0],
      "toolbar",
      document.createElement("div"),
    );
    const state = EditorState.create({
      doc,
      selection: EditorSelection.single(5),
    });
    const view = new EditorView({ state, parent: document.body });
    return { r, blocks, view };
  }

  it("requireHandler=true: returns false (falls through) when no handler set", () => {
    const { r, blocks, view } = setup();
    try {
      const km = makeFencedKeymap(() => blocks, r, [
        { key: "Mod-Enter", action: "onRun", requireHandler: true },
      ]);
      expect(km[0].run!(view)).toBe(false);
    } finally {
      view.destroy();
    }
  });

  it("requireHandler=false: returns true when block found even without handler", () => {
    const { r, blocks, view } = setup();
    try {
      const km = makeFencedKeymap(() => blocks, r, [
        { key: "Mod-Enter", action: "onRun", requireHandler: false },
      ]);
      expect(km[0].run!(view)).toBe(true);
    } finally {
      view.destroy();
    }
  });

  it("calls the handler when present + returns true", () => {
    const { r, blocks, view } = setup();
    try {
      const onRun = vi.fn();
      r.setBlockActions("fake_idx_0", { onRun });
      const km = makeFencedKeymap(() => blocks, r, [
        { key: "Mod-Enter", action: "onRun", requireHandler: true },
      ]);
      expect(km[0].run!(view)).toBe(true);
      expect(onRun).toHaveBeenCalled();
    } finally {
      view.destroy();
    }
  });

  it("returns false when the cursor is outside any block", () => {
    const r = mkRegistry();
    const blocks: FakeBlock[] = [];
    const state = EditorState.create({
      doc: "plain text",
      selection: EditorSelection.single(0),
    });
    const view = new EditorView({ state, parent: document.body });
    try {
      const km = makeFencedKeymap(() => blocks, r, [
        { key: "Mod-Enter", action: "onRun", requireHandler: false },
      ]);
      expect(km[0].run!(view)).toBe(false);
    } finally {
      view.destroy();
    }
  });
});

describe("createFencedBlockExtension", () => {
  it("builds a state field + keymap; rebuilds decorations on doc change", () => {
    const r = mkRegistry();
    let decorationCalls = 0;
    const ext = createFencedBlockExtension({
      scanner,
      registry: r,
      buildDecorations: () => {
        decorationCalls++;
        return Decoration.none;
      },
      keymapBindings: [
        { key: "Mod-Enter", action: "onRun", requireHandler: true },
      ],
    });
    const state = EditorState.create({
      doc: "intro\n```fake a\nbody\n```",
      extensions: [ext],
    });
    const view = new EditorView({ state, parent: document.body });
    try {
      expect(decorationCalls).toBe(1);
      // Dispatch via the view — that's what actually applies the
      // transaction and runs the StateField.update path.
      view.dispatch({
        changes: {
          from: view.state.doc.length,
          to: view.state.doc.length,
          insert: "\n",
        },
      });
      expect(decorationCalls).toBeGreaterThanOrEqual(2);
    } finally {
      view.destroy();
    }
  });

  it("re-decorates when the selection crosses a block boundary", () => {
    const r = mkRegistry();
    let calls = 0;
    const ext = createFencedBlockExtension({
      scanner,
      registry: r,
      buildDecorations: () => {
        calls++;
        return Decoration.none;
      },
      keymapBindings: [],
    });
    const doc = "intro\n```fake\nbody\n```\nepilogue";
    const state = EditorState.create({
      doc,
      selection: EditorSelection.single(0),
      extensions: [ext],
    });
    const view = new EditorView({ state, parent: document.body });
    try {
      const callsAfterCreate = calls;
      // Move cursor INTO the block via dispatch.
      view.dispatch({ selection: EditorSelection.single(10) });
      expect(calls).toBeGreaterThan(callsAfterCreate);
    } finally {
      view.destroy();
    }
  });

  it("does NOT re-decorate when the selection stays within the same region", () => {
    const r = mkRegistry();
    let calls = 0;
    const ext = createFencedBlockExtension({
      scanner,
      registry: r,
      buildDecorations: () => {
        calls++;
        return Decoration.none;
      },
      keymapBindings: [],
    });
    const state = EditorState.create({
      doc: "plain text doc with no fence",
      selection: EditorSelection.single(0),
      extensions: [ext],
    });
    const view = new EditorView({ state, parent: document.body });
    try {
      const before = calls;
      view.dispatch({ selection: EditorSelection.single(10) });
      // Cursor moved but no block boundaries crossed → no rebuild.
      expect(calls).toBe(before);
    } finally {
      view.destroy();
    }
  });

  it("includes extraExtensions when supplied (DB-style error field)", () => {
    const r = mkRegistry();
    const extraField = StateField.define<number>({
      create: () => 0,
      update: (v) => v,
    });
    const ext = createFencedBlockExtension({
      scanner,
      registry: r,
      buildDecorations: () => Decoration.none,
      keymapBindings: [],
      extraExtensions: () => [extraField],
    });
    const state = EditorState.create({
      doc: "x",
      extensions: [ext],
    });
    // Extra field's value should be readable from the state.
    expect(state.field(extraField)).toBe(0);
  });
});
