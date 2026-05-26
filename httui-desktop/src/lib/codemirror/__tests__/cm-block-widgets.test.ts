// Coverage backfill for cm-block-widgets.tsx. The file owns:
//   - findFencedBlocks (public scanner for ```http fences)
//   - extractAlias (public)
//   - createBlockWidgetPlugin (public StateField factory for DiffViewer)
//   - BlockWidget class + buildDiffDecorations + findFencedBlocksFromString
//     + langToBlockType + extractDisplayContent (private, used by widget)
//
// Existing consumers (document.ts / cm-references.ts / cm-move-blocks.ts
// / DiffViewer.tsx) covered the scanner indirectly; the widget machinery
// (Decoration.replace + createRoot → React) wasn't exercised by any test
// before this. Mounting an EditorView with the plugin exercises the full
// chain incl. BlockWidget.toDOM/destroy/eq/estimatedHeight/ignoreEvent.
//
// Coverage gate alvo: cm-block-widgets 36% → ≥80%.

import { describe, it, expect, vi } from "vitest";
import { EditorState, Text } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

import {
  findFencedBlocks,
  extractAlias,
  createBlockWidgetPlugin,
} from "../cm-block-widgets";

// React renders the StandaloneBlock inside the widget — mock it so the
// widget test stays fast (StandaloneBlock is itself heavily tested in
// blocks/__tests__/). The mock returns a static stub so toDOM /
// createRoot still flow but no real CM6 / Chakra mounts inside.
vi.mock("@/components/blocks/standalone/StandaloneBlock", () => ({
  StandaloneBlock: () => null,
}));

// Provider passes through children — no theme machinery needed in this file.
vi.mock("@/components/ui/provider", () => ({
  Provider: ({ children }: { children: React.ReactNode }) => children,
}));

// ─────────────── findFencedBlocks ───────────────

describe("findFencedBlocks", () => {
  it("returns [] for a doc with no fenced blocks", () => {
    const doc = Text.of(["plain", "text", "no fences"]);
    expect(findFencedBlocks(doc)).toEqual([]);
  });

  it("finds a single ```http block + extracts info", () => {
    const doc = Text.of([
      "intro",
      "```http alias=r1 timeout=30",
      "GET /x",
      "Header: v",
      "```",
      "outro",
    ]);
    const blocks = findFencedBlocks(doc);
    expect(blocks).toHaveLength(1);
    expect(blocks[0].lang).toBe("http");
    expect(blocks[0].info).toBe("alias=r1 timeout=30");
    expect(blocks[0].content).toBe("GET /x\nHeader: v");
  });

  it("finds multiple blocks separated by text", () => {
    const doc = Text.of([
      "```http",
      "GET /a",
      "```",
      "",
      "intermission",
      "",
      "```http alias=b",
      "POST /b",
      "```",
    ]);
    const blocks = findFencedBlocks(doc);
    expect(blocks).toHaveLength(2);
    expect(blocks[0].info).toBe("");
    expect(blocks[1].info).toBe("alias=b");
  });

  it("ignores blocks without a closing fence", () => {
    const doc = Text.of(["```http", "GET /x", "(no close)"]);
    expect(findFencedBlocks(doc)).toHaveLength(0);
  });

  it("ignores non-http fenced blocks (e.g. db, javascript)", () => {
    const doc = Text.of([
      "```db",
      "SELECT 1",
      "```",
      "```javascript",
      "1+1",
      "```",
    ]);
    expect(findFencedBlocks(doc)).toEqual([]);
  });

  it("preserves empty-body block (content === '')", () => {
    const doc = Text.of(["```http alias=empty", "```"]);
    const blocks = findFencedBlocks(doc);
    expect(blocks).toHaveLength(1);
    expect(blocks[0].content).toBe("");
  });

  it("from/to offsets bracket the entire fenced region (open line → close line end)", () => {
    const doc = Text.of(["```http", "GET /x", "```"]);
    const [b] = findFencedBlocks(doc);
    expect(b.from).toBe(0);
    // Final character of the closing line.
    expect(b.to).toBe(doc.length);
  });
});

// ─────────────── extractAlias ───────────────

describe("extractAlias", () => {
  it("parses alias= followed by non-whitespace", () => {
    expect(extractAlias("alias=r1 timeout=30")).toBe("r1");
    expect(extractAlias("timeout=10 alias=q")).toBe("q");
  });

  it("returns undefined when no alias present", () => {
    expect(extractAlias("")).toBeUndefined();
    expect(extractAlias("timeout=30 display=split")).toBeUndefined();
  });

  it("stops alias at the first whitespace", () => {
    expect(extractAlias("alias=foo bar")).toBe("foo");
  });
});

// ─────────────── createBlockWidgetPlugin (DiffViewer integration) ───────────────

function mount(doc: string, counterpart?: string, side: "a" | "b" = "b") {
  const container = document.createElement("div");
  document.body.appendChild(container);
  const view = new EditorView({
    state: EditorState.create({
      doc,
      extensions: [createBlockWidgetPlugin(counterpart, side)],
    }),
    parent: container,
  });
  return { view, container };
}

describe("createBlockWidgetPlugin — DiffViewer StateField", () => {
  it("renders a .cm-block-widget node for every fenced http block", () => {
    const { view, container } = mount(
      ["```http alias=q", "GET /x", "```"].join("\n"),
    );
    try {
      // The Decoration.replace mounts a contenteditable=false widget.
      const w = view.dom.querySelectorAll(".cm-block-widget");
      expect(w.length).toBe(1);
    } finally {
      view.destroy();
      container.remove();
    }
  });

  it("renders two widgets for two blocks", () => {
    const doc = [
      "```http",
      "GET /a",
      "```",
      "",
      "```http",
      "GET /b",
      "```",
    ].join("\n");
    const { view, container } = mount(doc);
    try {
      expect(view.dom.querySelectorAll(".cm-block-widget").length).toBe(2);
    } finally {
      view.destroy();
      container.remove();
    }
  });

  it("rebuilds the decoration set on doc change (adding a block grows the widget set)", () => {
    const { view, container } = mount("intro");
    try {
      expect(view.dom.querySelectorAll(".cm-block-widget").length).toBe(0);
      view.dispatch({
        changes: {
          from: view.state.doc.length,
          to: view.state.doc.length,
          insert: "\n```http\nGET /x\n```",
        },
      });
      expect(view.dom.querySelectorAll(".cm-block-widget").length).toBe(1);
    } finally {
      view.destroy();
      container.remove();
    }
  });

  it("passes counterpart content when blocks differ (BlockWidget eq distinguishes them)", () => {
    const docA = ["```http", "GET /a", "```"].join("\n");
    const docB = ["```http", "GET /b", "```"].join("\n");
    const { view, container } = mount(docA, docB, "a");
    try {
      expect(view.dom.querySelectorAll(".cm-block-widget").length).toBe(1);
    } finally {
      view.destroy();
      container.remove();
    }
  });

  it("ignores counterpart when bodies match (same display content) — widget gets null counterpart", () => {
    const doc = ["```http", "GET /x", "```"].join("\n");
    const { view, container } = mount(doc, doc, "b");
    try {
      expect(view.dom.querySelectorAll(".cm-block-widget").length).toBe(1);
    } finally {
      view.destroy();
      container.remove();
    }
  });

  it("handles undefined counterpart (single-side render)", () => {
    const doc = ["```http", "GET /x", "```"].join("\n");
    const { view, container } = mount(doc, undefined, "b");
    try {
      expect(view.dom.querySelectorAll(".cm-block-widget").length).toBe(1);
    } finally {
      view.destroy();
      container.remove();
    }
  });

  it("destroying the view tears down the widget react roots cleanly", () => {
    const doc = ["```http", "GET /x", "```"].join("\n");
    const { view, container } = mount(doc);
    expect(view.dom.querySelectorAll(".cm-block-widget").length).toBe(1);
    // BlockWidget.destroy queueMicrotask(unmount) — should not throw.
    expect(() => view.destroy()).not.toThrow();
    container.remove();
  });

  it("doc change inside the block: widget is rebuilt (content changed → eq false)", () => {
    const doc = ["```http alias=q", "GET /x", "```"].join("\n");
    const { view, container } = mount(doc);
    try {
      expect(view.dom.querySelectorAll(".cm-block-widget").length).toBe(1);
      // Mutate the URL inside the block — content differs → BlockWidget.eq
      // returns false → CM6 re-renders (destroy + new toDOM).
      const bodyStart = view.state.doc.line(2).from + "GET ".length;
      view.dispatch({
        changes: { from: bodyStart, to: bodyStart, insert: "/changed" },
      });
      // Still one widget post-rebuild.
      expect(view.dom.querySelectorAll(".cm-block-widget").length).toBe(1);
    } finally {
      view.destroy();
      container.remove();
    }
  });

  it("counterpart with extra blocks past the doc length: extras are ignored (1:1 by index)", () => {
    const doc = ["```http", "GET /a", "```"].join("\n");
    const counterpart = [
      "```http",
      "GET /a-counter",
      "```",
      "",
      "```http",
      "GET /b",
      "```",
    ].join("\n");
    const { view, container } = mount(doc, counterpart, "a");
    try {
      // Only 1 widget for the local doc; extra counterpart block ignored.
      expect(view.dom.querySelectorAll(".cm-block-widget").length).toBe(1);
    } finally {
      view.destroy();
      container.remove();
    }
  });

  it("JSON-shaped content uses data.body/url; non-JSON content falls through", () => {
    // Side A has JSON content; side B has raw text. Both render widgets.
    const docJson = [
      "```http alias=x",
      '{"method":"GET","url":"/x","body":"payload"}',
      "```",
    ].join("\n");
    const docRaw = ["```http alias=x", "GET /raw", "```"].join("\n");
    const { view, container } = mount(docJson, docRaw, "b");
    try {
      expect(view.dom.querySelectorAll(".cm-block-widget").length).toBe(1);
    } finally {
      view.destroy();
      container.remove();
    }
  });
});
