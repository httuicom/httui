import { describe, it, expect, vi } from "vitest";
import { EditorState, EditorSelection } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import type {
  Completion,
  CompletionContext,
  CompletionResult,
} from "@codemirror/autocomplete";

// Mock the block-registry so slash commands load without pulling
// HTTP/DB extension factories through. The mocked spec contributes
// 2 stub slash entries + 1 icon, enough to exercise the assembly.
vi.mock("@/lib/blocks/block-registry", () => ({
  getRegisteredBlockSlashCommands: (section: unknown) => [
    {
      label: "Mocked Block",
      type: "database",
      insert: "```mock\n\n```\n",
      cursorOffset: -5,
      section,
    },
  ],
  getRegisteredBlockIcons: () => ({
    database: '<rect x="0" y="0" width="10" height="10"/>',
  }),
}));

import {
  slashCommands,
  slashCompletionSource,
  slashIconOption,
} from "@/lib/codemirror/cm-slash-commands";

// ── Helpers ────────────────────────────────────────────────────

function makeCtx(
  doc: string,
  pos: number,
  opts: { explicit?: boolean } = {},
): CompletionContext {
  const state = EditorState.create({
    doc,
    selection: EditorSelection.single(pos),
  });
  return {
    state,
    pos,
    explicit: opts.explicit ?? true,
    matchBefore: () => null, // unused by slashCompletionSource
    aborted: false,
  } as unknown as CompletionContext;
}

// ── slashCompletionSource ──────────────────────────────────────

describe("slashCompletionSource", () => {
  it("returns null when the cursor isn't preceded by '/' at line start", () => {
    const doc = "no slash here";
    expect(slashCompletionSource(makeCtx(doc, doc.length))).toBeNull();
  });

  it("returns null when the line has non-whitespace before '/'", () => {
    const doc = "hello /h";
    expect(slashCompletionSource(makeCtx(doc, doc.length))).toBeNull();
  });

  it("returns options for '/' with empty query (all entries match)", () => {
    const doc = "/";
    const result = slashCompletionSource(makeCtx(doc, doc.length));
    expect(result).not.toBeNull();
    const r = result as CompletionResult;
    expect(r.options.length).toBeGreaterThan(5);
    expect(r.filter).toBe(false);
    // Standard COMMANDS (Heading 1, Bulleted list, …) plus the
    // registry's mocked block entry.
    const labels = r.options.map((o) => o.displayLabel);
    expect(labels).toContain("Heading 1");
    expect(labels).toContain("Mocked Block");
  });

  it("filters labels case-insensitively against the query", () => {
    const doc = "/head";
    const r = slashCompletionSource(makeCtx(doc, doc.length))!;
    const labels = r.options.map((o) => o.displayLabel);
    expect(labels).toEqual(
      expect.arrayContaining(["Heading 1", "Heading 2", "Heading 3"]),
    );
    expect(labels.every((l) => /head/i.test(l as string))).toBe(true);
  });

  it("accepts leading whitespace on the line and sets `from` accordingly", () => {
    const doc = "   /he";
    const r = slashCompletionSource(makeCtx(doc, doc.length))!;
    // `from` is line.from (0) + prefix.length (3 spaces) = 3.
    expect(r.from).toBe(3);
  });

  it("returns null when no entry matches the query", () => {
    const doc = "/zzzzzzz_no_such_block";
    expect(slashCompletionSource(makeCtx(doc, doc.length))).toBeNull();
  });

  it("each option carries displayLabel, type, section, apply", () => {
    const doc = "/heading 1";
    const r = slashCompletionSource(makeCtx(doc, doc.length))!;
    const heading1 = r.options.find((o) => o.displayLabel === "Heading 1");
    expect(heading1).toBeDefined();
    expect(heading1!.type).toBe("h1");
    expect(typeof heading1!.apply).toBe("function");
    expect(heading1!.label).toBe("/Heading 1");
  });
});

// ── option.apply (dispatch into a real EditorView) ─────────────

describe("slashCompletionSource option.apply", () => {
  it("dispatches changes + selection to the EditorView", () => {
    // Use "/heading 1" (substring match, not fuzzy) — see slashCompletionSource
    // L197: `cmd.label.toLowerCase().includes(query)`.
    const doc = "/heading 1";
    const state = EditorState.create({
      doc,
      selection: EditorSelection.single(doc.length),
    });
    const view = new EditorView({ state, parent: document.body });
    try {
      const ctx = makeCtx(doc, doc.length);
      const r = slashCompletionSource(ctx)!;
      const h1 = r.options.find((o) => o.displayLabel === "Heading 1")!;
      // Mimic CM6 invocation: apply replaces (from .. cursor) with insert.
      const applyFn = h1.apply as (
        v: EditorView,
        c: Completion,
        from: number,
        to: number,
      ) => void;
      applyFn(view, h1, 0, doc.length);
      expect(view.state.doc.toString()).toBe("# ");
      // Cursor lands at insert length + cursorOffset (none here → end).
      expect(view.state.selection.main.head).toBe(2);
    } finally {
      view.destroy();
    }
  });

  it("respects cursorOffset (e.g. inline math '$x^2$' rewinds 1)", () => {
    const doc = "/inline";
    const state = EditorState.create({
      doc,
      selection: EditorSelection.single(doc.length),
    });
    const view = new EditorView({ state, parent: document.body });
    try {
      const r = slashCompletionSource(makeCtx(doc, doc.length))!;
      const inlineMath = r.options.find(
        (o) => o.displayLabel === "Inline formula",
      )!;
      (
        inlineMath.apply as (
          v: EditorView,
          c: Completion,
          from: number,
          to: number,
        ) => void
      )(view, inlineMath, 0, doc.length);
      // insert = "$x^2$" (5 chars), cursorOffset = -1 → caret at 4.
      expect(view.state.doc.toString()).toBe("$x^2$");
      expect(view.state.selection.main.head).toBe(4);
    } finally {
      view.destroy();
    }
  });
});

// ── slashIconOption renderer ───────────────────────────────────

describe("slashIconOption.render", () => {
  it("position is 20 (before label + detail per addToOptions API)", () => {
    expect(slashIconOption.position).toBe(20);
  });

  it("returns null for completions without a type", () => {
    const out = slashIconOption.render({
      label: "x",
    } as unknown as Completion);
    expect(out).toBeNull();
  });

  it("returns null when the type has no registered SVG", () => {
    const out = slashIconOption.render({
      label: "x",
      type: "no-such-icon",
    } as unknown as Completion);
    expect(out).toBeNull();
  });

  it("returns a span.cm-slash-icon wrapping an <svg> for a known type", () => {
    const out = slashIconOption.render({
      label: "Heading 1",
      type: "h1",
    } as unknown as Completion);
    expect(out).toBeInstanceOf(HTMLElement);
    expect(out!.className).toBe("cm-slash-icon");
    const svg = out!.querySelector("svg");
    expect(svg).not.toBeNull();
    expect(svg!.getAttribute("width")).toBe("18");
    expect(svg!.getAttribute("viewBox")).toBe("0 0 24 24");
  });

  it("renders the registry-contributed icon (block type)", () => {
    const out = slashIconOption.render({
      label: "Mocked Block",
      type: "database",
    } as unknown as Completion);
    expect(out).not.toBeNull();
    const svg = out!.querySelector("svg");
    expect(svg).not.toBeNull();
    expect(svg!.innerHTML).toContain("rect");
  });
});

// ── slashCommands() extension factory ──────────────────────────

describe("slashCommands extension", () => {
  it("returns a CM6 extension array (theme only — completion is wired separately)", () => {
    const ext = slashCommands();
    // Extension can be a value or an array; either way it's truthy.
    expect(ext).toBeDefined();
  });
});
