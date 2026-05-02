// Vim doc-line motions for MarkdownEditor.
//
// CM6's default cursorLineUp/Down is pixel-based — it teleports to
// "Ln 1, Col 1" when there's a tall block widget (like
// DbClosePanelWidget) between the current line and the next text line
// because moveVertically can't find a text line at the target y.
// This module provides:
//
//   • `vimCompartment` — exported so MarkdownEditor can reconfigure it
//     when the user toggles vim mode without re-creating the editor.
//   • `docLineNavKeymap` — high-precedence ArrowUp/Down keymap that
//     walks by document line. Bails out when vim owns motion (normal /
//     visual mode) so vim keeps ownership of h/j/k/l motion.
//   • `installDocLineVimMotions()` — replaces vim's `moveByLines`
//     motion with a doc-line variant. Idempotent. Called once at
//     module import time so j/k/<Up>/<Down>/+/-/_ also avoid the
//     pixel-based teleport in normal/visual mode.

import { keymap, EditorView } from "@codemirror/view";
import { Compartment, EditorSelection, Prec } from "@codemirror/state";
import {
  Vim,
  getCM,
  type CodeMirrorV,
  type MotionArgs,
  type Pos,
  type vimState,
} from "@replit/codemirror-vim";

// Compartment for toggling vim mode without recreating the editor.
export const vimCompartment = new Compartment();

// Vim-aware guard: bail out when vim is active in a non-insert mode so
// vim keeps ownership of h/j/k/l / arrow motion / visual selection. In
// insert mode and when vim is off, we take over ArrowUp/Down to navigate
// by doc line.
export function vimOwnsMotion(view: EditorView): boolean {
  const cm = getCM(view);
  const v = cm?.state.vim;
  if (!v) return false;
  return !v.insertMode;
}

export const docLineNavKeymap = Prec.high(
  keymap.of([
    {
      key: "ArrowUp",
      run: (view) => {
        if (vimOwnsMotion(view)) return false;
        const sel = view.state.selection.main;
        if (!sel.empty) return false;
        const doc = view.state.doc;
        const line = doc.lineAt(sel.head);
        if (line.number === 1) return false;
        const prev = doc.line(line.number - 1);
        const col = sel.head - line.from;
        const target = Math.min(prev.from + col, prev.to);
        view.dispatch({
          selection: EditorSelection.cursor(target),
          scrollIntoView: true,
        });
        return true;
      },
    },
    {
      key: "ArrowDown",
      run: (view) => {
        if (vimOwnsMotion(view)) return false;
        const sel = view.state.selection.main;
        if (!sel.empty) return false;
        const doc = view.state.doc;
        const line = doc.lineAt(sel.head);
        if (line.number === doc.lines) return false;
        const next = doc.line(line.number + 1);
        const col = sel.head - line.from;
        const target = Math.min(next.from + col, next.to);
        view.dispatch({
          selection: EditorSelection.cursor(target),
          scrollIntoView: true,
        });
        return true;
      },
    },
  ]),
);

// Replace vim's built-in `moveByLines` motion with a doc-line variant.
// The upstream implementation uses `cm.findPosV(..., 'line', ...)` which
// the CM5→CM6 bridge routes through `moveVertically` — pixel-based
// motion that teleports through tall block widgets. Because the vim
// dispatcher looks motions up by name, a single defineMotion call here
// transparently fixes j, k, <Up>, <Down>, +, -, _ in normal / visual.
//
// Why: keeps normal/visual vim state intact (HPos stickiness, visual
// selection extension) while replacing only the vertical-motion compute.
let vimMotionsInstalled = false;

export function installDocLineVimMotions(): void {
  if (vimMotionsInstalled) return;
  vimMotionsInstalled = true;
  const docMoveByLines = function (
    cm: CodeMirrorV,
    head: Pos,
    motionArgs: MotionArgs,
    state: vimState,
  ): Pos {
    let endCh = head.ch;
    // HPos stickiness for j/k chains. Any other motion (h/l, word, gj)
    // resets the goal column — minor regression vs. vanilla vim that we
    // accept in exchange for not teleporting through widgets.
    if (state.lastMotion === docMoveByLines) {
      endCh = state.lastHPos ?? head.ch;
    } else {
      state.lastHPos = endCh;
    }
    const repeat = motionArgs.repeat + (motionArgs.repeatOffset || 0);
    const first = cm.firstLine();
    const last = cm.lastLine();
    let line = motionArgs.forward ? head.line + repeat : head.line - repeat;
    if (line < first) line = first;
    if (line > last) line = last;
    if (motionArgs.toFirstChar) {
      const text: string = cm.getLine(line) ?? "";
      const match = /^\s*/.exec(text);
      endCh = match ? match[0].length : 0;
      state.lastHPos = endCh;
    }
    const lineText: string = cm.getLine(line) ?? "";
    if (endCh > lineText.length) endCh = lineText.length;
    try {
      state.lastHSPos = cm.charCoords({ line, ch: endCh }, "div").left;
    } catch {
      // charCoords can throw before the view is laid out; HSPos is only
      // used by gj/gk, which we don't override. Safe to ignore.
    }
    return { line, ch: endCh };
  };
  Vim.defineMotion("moveByLines", docMoveByLines);
}

// Test-only: reset the install guard so unit tests can verify
// idempotency. Not exported from the package surface in practice.
export function __resetVimMotionsInstalled(): void {
  vimMotionsInstalled = false;
}
