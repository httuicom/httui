import { type Extension } from "@codemirror/state";
import { keymap, EditorView } from "@codemirror/view";
import { findFencedBlocks } from "./cm-block-widgets";

/**
 * Find the fenced block that contains or is immediately adjacent to a position.
 * Returns the block if the cursor is on the line just before, inside, or just after it.
 */
function findBlockNearPos(view: EditorView, pos: number) {
  const blocks = findFencedBlocks(view.state.doc);
  const cursorLine = view.state.doc.lineAt(pos).number;

  for (const block of blocks) {
    const blockStartLine = view.state.doc.lineAt(block.from).number;
    const blockEndLine = view.state.doc.lineAt(block.to).number;

    // Cursor is on the line just before the block, inside it, or just after
    if (cursorLine >= blockStartLine - 1 && cursorLine <= blockEndLine + 1) {
      return block;
    }
  }
  return null;
}

/**
 * Move a block up: swap it with the content above it.
 */
function moveBlockUp(view: EditorView): boolean {
  const block = findBlockNearPos(view, view.state.selection.main.head);
  if (!block) return false;

  const doc = view.state.doc;
  const blockStartLine = doc.lineAt(block.from);

  // Can't move up if already at the top
  if (blockStartLine.number <= 1) return false;

  // Find the line above the block
  const lineAbove = doc.line(blockStartLine.number - 1);

  // Get block text including trailing newline
  const blockEnd = block.to < doc.length ? block.to + 1 : block.to;
  const blockText = doc.sliceString(block.from, blockEnd);

  // Get the line above text including its newline
  const aboveText = doc.sliceString(lineAbove.from, blockStartLine.from);

  // Swap: replace the range [lineAbove.from, blockEnd] with blockText + aboveText
  const newText = blockText + aboveText.replace(/\n$/, "");
  view.dispatch({
    changes: { from: lineAbove.from, to: blockEnd, insert: newText },
    selection: { anchor: lineAbove.from },
  });

  return true;
}

/**
 * Move a block down: swap it with the content below it.
 */
function moveBlockDown(view: EditorView): boolean {
  const block = findBlockNearPos(view, view.state.selection.main.head);
  if (!block) return false;

  const doc = view.state.doc;
  const blockEndLine = doc.lineAt(block.to);

  // Can't move down if already at the bottom
  if (blockEndLine.number >= doc.lines) return false;

  // Find the line below the block
  const lineBelow = doc.line(blockEndLine.number + 1);

  // Get block text including trailing newline
  const blockEnd = block.to < doc.length ? block.to + 1 : block.to;
  const blockText = doc.sliceString(block.from, blockEnd);

  // Get the line below text including its newline
  const belowEnd = lineBelow.to < doc.length ? lineBelow.to + 1 : lineBelow.to;
  const belowText = doc.sliceString(blockEnd, belowEnd);

  // Swap: replace the range [block.from, belowEnd] with belowText + blockText
  const newText = belowText + blockText.replace(/\n$/, "");
  view.dispatch({
    changes: { from: block.from, to: belowEnd, insert: newText },
    selection: { anchor: block.from + belowText.length },
  });

  return true;
}

/** Keymap for moving blocks with Alt+Up/Down */
export function moveBlocksKeymap(): Extension {
  return keymap.of([
    { key: "Alt-ArrowUp", run: moveBlockUp },
    { key: "Alt-ArrowDown", run: moveBlockDown },
  ]);
}
