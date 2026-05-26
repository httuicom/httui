import { RangeSetBuilder, StateField, Text as CMText } from "@codemirror/state";
import {
  Decoration,
  WidgetType,
  type DecorationSet,
  EditorView,
} from "@codemirror/view";
import { createRoot, type Root } from "react-dom/client";
import { Provider } from "@/components/ui/provider";
import { StandaloneBlock } from "@/components/blocks/standalone/StandaloneBlock";

// Scans `http` fences only (db has its own scanner in cm-db-block.tsx).
// Consumed by the read-only DiffViewer (`createBlockWidgetPlugin`) and by
// document / reference / move-block helpers that call `findFencedBlocks`.
// The editor-side portal pipeline for the removed E2E block was dropped
// (it had been a no-op since E2E removal).
const BLOCK_OPEN_RE = /^```(http)(.*)$/;
const BLOCK_CLOSE_RE = /^```\s*$/;

export interface FencedBlock {
  from: number;
  to: number;
  lang: string;
  info: string;
  content: string;
}

/** Scan a CodeMirror document for fenced executable blocks. */
export function findFencedBlocks(doc: CMText): FencedBlock[] {
  const blocks: FencedBlock[] = [];
  let inBlock = false;
  let blockStart = 0;
  let lang = "";
  let info = "";
  let contentLines: string[] = [];

  for (let i = 1; i <= doc.lines; i++) {
    const line = doc.line(i);
    const text = line.text;

    if (!inBlock) {
      const match = text.match(BLOCK_OPEN_RE);
      if (match) {
        inBlock = true;
        blockStart = line.from;
        lang = match[1];
        info = match[2].trim();
        contentLines = [];
      }
    } else {
      if (BLOCK_CLOSE_RE.test(text)) {
        blocks.push({
          from: blockStart,
          to: line.to,
          lang,
          info,
          content: contentLines.join("\n"),
        });
        inBlock = false;
      } else {
        contentLines.push(text);
      }
    }
  }

  return blocks;
}

/** Extract alias from info string */
export function extractAlias(info: string): string | undefined {
  const match = info.match(/alias=(\S+)/);
  return match?.[1];
}

/** Map language string to block type */
function langToBlockType(lang: string): string {
  if (lang === "http") return "http";
  return lang;
}

/** Extract display content (e.g. body/url) from JSON-serialized block content */
function extractDisplayContent(blockType: string, raw: string): string {
  try {
    const data = JSON.parse(raw);
    if (blockType === "http") return data.body ?? data.url ?? raw;
    return JSON.stringify(data, null, 2);
  } catch {
    return raw;
  }
}

/** Parse fenced blocks from raw markdown string */
function findFencedBlocksFromString(markdown: string): FencedBlock[] {
  const doc = CMText.of(markdown.split("\n"));
  return findFencedBlocks(doc);
}

// ── DiffViewer widgets (read-only, uses Decoration.replace + createRoot) ─────

class BlockWidget extends WidgetType {
  private root: Root | null = null;

  constructor(
    readonly lang: string,
    readonly info: string,
    readonly content: string,
    readonly counterpartContent: string | null,
    readonly side: "a" | "b",
  ) {
    super();
  }

  toDOM(): HTMLElement {
    const container = document.createElement("div");
    container.className = "cm-block-widget";
    container.contentEditable = "false";
    container.style.padding = "2px 0";
    container.style.overflow = "hidden";
    container.style.maxWidth = "100%";

    this.root = createRoot(container);
    this.root.render(
      <Provider>
        <StandaloneBlock
          blockType={langToBlockType(this.lang)}
          content={this.content}
          counterpartContent={this.counterpartContent ?? undefined}
          side={this.side}
          alias={extractAlias(this.info)}
        />
      </Provider>,
    );

    return container;
  }

  destroy(): void {
    if (this.root) {
      const root = this.root;
      this.root = null;
      queueMicrotask(() => root.unmount());
    }
  }

  eq(other: BlockWidget): boolean {
    return (
      this.lang === other.lang &&
      this.content === other.content &&
      this.info === other.info &&
      this.counterpartContent === other.counterpartContent
    );
  }

  get estimatedHeight(): number {
    return 150;
  }

  ignoreEvent(): boolean {
    return true;
  }
}

function buildDiffDecorations(
  doc: CMText,
  counterpartBlocks: FencedBlock[],
  side: "a" | "b",
): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>();
  const blocks = findFencedBlocks(doc);

  for (let i = 0; i < blocks.length; i++) {
    const block = blocks[i];
    const counterpart = counterpartBlocks[i];
    const blockType = langToBlockType(block.lang);

    const thisDisplay = extractDisplayContent(blockType, block.content);
    const counterpartDisplay = counterpart
      ? extractDisplayContent(
          langToBlockType(counterpart.lang),
          counterpart.content,
        )
      : null;

    builder.add(
      block.from,
      block.to,
      Decoration.replace({
        widget: new BlockWidget(
          block.lang,
          block.info,
          block.content,
          counterpartDisplay !== thisDisplay ? counterpartDisplay : null,
          side,
        ),
        block: true,
      }),
    );
  }

  return builder.finish();
}

/**
 * Create a CodeMirror extension for the DiffViewer (read-only).
 * Uses Decoration.replace + createRoot (fine for read-only context).
 */
export function createBlockWidgetPlugin(
  counterpartMarkdown: string | undefined,
  side: "a" | "b",
) {
  const counterpartBlocks = counterpartMarkdown
    ? findFencedBlocksFromString(counterpartMarkdown)
    : [];

  return StateField.define<DecorationSet>({
    create(state) {
      return buildDiffDecorations(state.doc, counterpartBlocks, side);
    },
    update(decos, tr) {
      if (tr.docChanged) {
        return buildDiffDecorations(tr.state.doc, counterpartBlocks, side);
      }
      return decos;
    },
    provide: (f) => EditorView.decorations.from(f),
  });
}
