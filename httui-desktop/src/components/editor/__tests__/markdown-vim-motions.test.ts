import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { EditorState, EditorSelection } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

vi.mock("@replit/codemirror-vim", () => ({
  Vim: { defineMotion: vi.fn() },
  getCM: vi.fn(),
}));

import { Vim, getCM } from "@replit/codemirror-vim";
import {
  vimCompartment,
  vimOwnsMotion,
  docLineNavKeymap,
  installDocLineVimMotions,
  __resetVimMotionsInstalled,
} from "@/components/editor/markdown-vim-motions";

function makeView(doc: string, headOffset: number): EditorView {
  const state = EditorState.create({
    doc,
    selection: EditorSelection.cursor(headOffset),
    extensions: [docLineNavKeymap],
  });
  return new EditorView({ state });
}

describe("markdown-vim-motions", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    __resetVimMotionsInstalled();
  });

  afterEach(() => {
    __resetVimMotionsInstalled();
  });

  describe("vimCompartment", () => {
    it("is exported as a CM6 Compartment instance", () => {
      // Compartment#of accepts a single Extension and returns one too —
      // the simplest behavioral check.
      expect(typeof vimCompartment.of).toBe("function");
      expect(vimCompartment.of([])).toBeDefined();
    });
  });

  describe("vimOwnsMotion", () => {
    it("returns false when getCM returns null (vim not active)", () => {
      vi.mocked(getCM).mockReturnValue(null);
      const view = makeView("abc", 0);
      expect(vimOwnsMotion(view)).toBe(false);
      view.destroy();
    });

    it("returns false when vim is in insert mode", () => {
      vi.mocked(getCM).mockReturnValue({
        state: { vim: { insertMode: true } },
      } as unknown as ReturnType<typeof getCM>);
      const view = makeView("abc", 0);
      expect(vimOwnsMotion(view)).toBe(false);
      view.destroy();
    });

    it("returns true when vim is in normal/visual (non-insert) mode", () => {
      vi.mocked(getCM).mockReturnValue({
        state: { vim: { insertMode: false } },
      } as unknown as ReturnType<typeof getCM>);
      const view = makeView("abc", 0);
      expect(vimOwnsMotion(view)).toBe(true);
      view.destroy();
    });

    it("returns false when vim state is missing entirely", () => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      vi.mocked(getCM).mockReturnValue({ state: {} } as any);
      const view = makeView("abc", 0);
      expect(vimOwnsMotion(view)).toBe(false);
      view.destroy();
    });
  });

  describe("docLineNavKeymap — ArrowDown", () => {
    beforeEach(() => {
      // Simulate vim off: getCM returns null → vimOwnsMotion === false
      vi.mocked(getCM).mockReturnValue(null);
    });

    it("walks to the next document line preserving column", () => {
      const view = makeView("first\nsecond\nthird", 2 /* "fi|rst" */);
      const handler = (view as unknown as { contentDOM: HTMLElement })
        .contentDOM;
      // Synthesize ArrowDown via the keymap dispatcher on the view.
      const evt = new KeyboardEvent("keydown", { key: "ArrowDown" });
      handler.dispatchEvent(evt);
      // Cursor moved from line 1 col 2 → line 2 col 2 (within "second")
      expect(view.state.selection.main.head).toBe(8);
      view.destroy();
    });

    it("clamps to end-of-line when target line is shorter", () => {
      const view = makeView("longerline\nab", 8 /* near end of line 1 */);
      const evt = new KeyboardEvent("keydown", { key: "ArrowDown" });
      (view as unknown as { contentDOM: HTMLElement }).contentDOM.dispatchEvent(
        evt,
      );
      // Line 2 ("ab") is only 2 chars; cursor lands at its end (offset 13).
      expect(view.state.selection.main.head).toBe(13);
      view.destroy();
    });

    it("does nothing on the last line", () => {
      const view = makeView("a\nb", 2 /* on "b" */);
      const evt = new KeyboardEvent("keydown", { key: "ArrowDown" });
      (view as unknown as { contentDOM: HTMLElement }).contentDOM.dispatchEvent(
        evt,
      );
      // Still at offset 2 (no change).
      expect(view.state.selection.main.head).toBe(2);
      view.destroy();
    });

    it("does nothing when there is a non-empty selection", () => {
      const state = EditorState.create({
        doc: "first\nsecond",
        selection: EditorSelection.range(0, 3),
        extensions: [docLineNavKeymap],
      });
      const view = new EditorView({ state });
      const evt = new KeyboardEvent("keydown", { key: "ArrowDown" });
      (view as unknown as { contentDOM: HTMLElement }).contentDOM.dispatchEvent(
        evt,
      );
      expect(view.state.selection.main.from).toBe(0);
      expect(view.state.selection.main.to).toBe(3);
      view.destroy();
    });

    it("yields to vim when vim owns motion (normal mode)", () => {
      vi.mocked(getCM).mockReturnValue({
        state: { vim: { insertMode: false } },
      } as unknown as ReturnType<typeof getCM>);
      const view = makeView("first\nsecond", 2);
      const evt = new KeyboardEvent("keydown", { key: "ArrowDown" });
      (view as unknown as { contentDOM: HTMLElement }).contentDOM.dispatchEvent(
        evt,
      );
      // Cursor unchanged: doc-line keymap returned false, vim handles
      // the key (mocked away in unit test, so no-op here).
      expect(view.state.selection.main.head).toBe(2);
      view.destroy();
    });
  });

  describe("docLineNavKeymap — ArrowUp", () => {
    beforeEach(() => {
      vi.mocked(getCM).mockReturnValue(null);
    });

    it("walks to the previous document line preserving column", () => {
      const view = makeView("first\nsecond", 9 /* "se|cond" col 3 */);
      const evt = new KeyboardEvent("keydown", { key: "ArrowUp" });
      (view as unknown as { contentDOM: HTMLElement }).contentDOM.dispatchEvent(
        evt,
      );
      // Line 1 col 3
      expect(view.state.selection.main.head).toBe(3);
      view.destroy();
    });

    it("does nothing on the first line", () => {
      const view = makeView("only line", 3);
      const evt = new KeyboardEvent("keydown", { key: "ArrowUp" });
      (view as unknown as { contentDOM: HTMLElement }).contentDOM.dispatchEvent(
        evt,
      );
      expect(view.state.selection.main.head).toBe(3);
      view.destroy();
    });

    it("clamps to end-of-line on the previous line if shorter", () => {
      const view = makeView("ab\nlongerline", 10 /* "longer|line" */);
      const evt = new KeyboardEvent("keydown", { key: "ArrowUp" });
      (view as unknown as { contentDOM: HTMLElement }).contentDOM.dispatchEvent(
        evt,
      );
      // Previous line ("ab") only has 2 chars; cursor lands at offset 2.
      expect(view.state.selection.main.head).toBe(2);
      view.destroy();
    });

    it("yields to vim when vim owns motion (normal mode)", () => {
      vi.mocked(getCM).mockReturnValue({
        state: { vim: { insertMode: false } },
      } as unknown as ReturnType<typeof getCM>);
      const view = makeView("first\nsecond", 9);
      const evt = new KeyboardEvent("keydown", { key: "ArrowUp" });
      (view as unknown as { contentDOM: HTMLElement }).contentDOM.dispatchEvent(
        evt,
      );
      // Cursor unchanged: vim owns the key.
      expect(view.state.selection.main.head).toBe(9);
      view.destroy();
    });

    it("does nothing when there is a non-empty selection", () => {
      const state = EditorState.create({
        doc: "first\nsecond",
        selection: EditorSelection.range(9, 11),
        extensions: [docLineNavKeymap],
      });
      const view = new EditorView({ state });
      const evt = new KeyboardEvent("keydown", { key: "ArrowUp" });
      (view as unknown as { contentDOM: HTMLElement }).contentDOM.dispatchEvent(
        evt,
      );
      expect(view.state.selection.main.from).toBe(9);
      expect(view.state.selection.main.to).toBe(11);
      view.destroy();
    });
  });

  describe("installDocLineVimMotions", () => {
    it("calls Vim.defineMotion with 'moveByLines' on first invocation", () => {
      installDocLineVimMotions();
      expect(Vim.defineMotion).toHaveBeenCalledTimes(1);
      expect(vi.mocked(Vim.defineMotion).mock.calls[0][0]).toBe("moveByLines");
    });

    it("is idempotent — repeat calls do NOT re-register", () => {
      installDocLineVimMotions();
      installDocLineVimMotions();
      installDocLineVimMotions();
      expect(Vim.defineMotion).toHaveBeenCalledTimes(1);
    });

    describe("docMoveByLines motion", () => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      let capturedMotion: any;

      beforeEach(() => {
        installDocLineVimMotions();
        capturedMotion = vi.mocked(Vim.defineMotion).mock.calls[0][1];
      });

      it("moves forward by repeat count, clamping to last line", () => {
        const cm = {
          firstLine: () => 0,
          lastLine: () => 5,
          getLine: (n: number) => `line ${n}`,
          charCoords: () => ({ left: 0 }),
        };
        const state = { lastMotion: null, lastHPos: null };
        const result = capturedMotion(
          cm,
          { line: 1, ch: 2 },
          { repeat: 100, forward: true },
          state,
        );
        expect(result.line).toBe(5);
        expect(result.ch).toBe(2);
      });

      it("moves backward, clamping to first line", () => {
        const cm = {
          firstLine: () => 0,
          lastLine: () => 5,
          getLine: (n: number) => `line ${n}`,
          charCoords: () => ({ left: 0 }),
        };
        const state = { lastMotion: null, lastHPos: null };
        const result = capturedMotion(
          cm,
          { line: 2, ch: 1 },
          { repeat: 100, forward: false },
          state,
        );
        expect(result.line).toBe(0);
        expect(result.ch).toBe(1);
      });

      it("respects HPos stickiness when chained from itself", () => {
        const cm = {
          firstLine: () => 0,
          lastLine: () => 5,
          getLine: (n: number) => "x".repeat(10 + n),
          charCoords: () => ({ left: 0 }),
        };
        const state = {
          lastMotion: capturedMotion,
          lastHPos: 7,
        };
        const result = capturedMotion(
          cm,
          { line: 1, ch: 3 },
          { repeat: 1, forward: true },
          state,
        );
        // ch comes from lastHPos (7), not head.ch (3)
        expect(result.ch).toBe(7);
      });

      it("clamps endCh to line text length", () => {
        const cm = {
          firstLine: () => 0,
          lastLine: () => 5,
          getLine: (n: number) => (n === 2 ? "abc" : "longerline"),
          charCoords: () => ({ left: 0 }),
        };
        const state = { lastMotion: null, lastHPos: null };
        const result = capturedMotion(
          cm,
          { line: 1, ch: 7 },
          { repeat: 1, forward: true },
          state,
        );
        // Target line 2 ("abc") only has 3 chars; ch clamped.
        expect(result.line).toBe(2);
        expect(result.ch).toBe(3);
      });

      it("toFirstChar lands on indentation length", () => {
        const cm = {
          firstLine: () => 0,
          lastLine: () => 5,
          getLine: (n: number) => (n === 2 ? "    indented" : "abc"),
          charCoords: () => ({ left: 0 }),
        };
        const state = { lastMotion: null, lastHPos: null };
        const result = capturedMotion(
          cm,
          { line: 1, ch: 0 },
          { repeat: 1, forward: true, toFirstChar: true },
          state,
        );
        expect(result.line).toBe(2);
        expect(result.ch).toBe(4);
        expect(state.lastHPos).toBe(4);
      });

      it("survives charCoords throwing", () => {
        const cm = {
          firstLine: () => 0,
          lastLine: () => 5,
          getLine: (n: number) => `line ${n}`,
          charCoords: () => {
            throw new Error("layout not ready");
          },
        };
        const state = { lastMotion: null, lastHPos: null };
        expect(() =>
          capturedMotion(
            cm,
            { line: 1, ch: 0 },
            { repeat: 1, forward: true },
            state,
          ),
        ).not.toThrow();
      });

      it("handles missing getLine result gracefully", () => {
        const cm = {
          firstLine: () => 0,
          lastLine: () => 5,
          getLine: () => undefined,
          charCoords: () => ({ left: 0 }),
        };
        const state = { lastMotion: null, lastHPos: null };
        const result = capturedMotion(
          cm,
          { line: 0, ch: 5 },
          { repeat: 1, forward: true },
          state,
        );
        // ch clamped to 0 (empty line)
        expect(result.ch).toBe(0);
      });

      it("respects repeatOffset additive to repeat", () => {
        const cm = {
          firstLine: () => 0,
          lastLine: () => 10,
          getLine: (n: number) => `line ${n}`,
          charCoords: () => ({ left: 0 }),
        };
        const state = { lastMotion: null, lastHPos: null };
        const result = capturedMotion(
          cm,
          { line: 1, ch: 0 },
          { repeat: 2, repeatOffset: 1, forward: true },
          state,
        );
        // 1 + 3 = 4
        expect(result.line).toBe(4);
      });
    });
  });
});
