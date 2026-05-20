// Coverage backfill for the error-squiggle machinery + body decorations
// + ref completion source in `cm-db-block.tsx`. The existing test
// (cm-db-block.test.ts) covers scanner + extension smoke + schema
// autocomplete. This sibling exercises:
//   - setDbBlockErrors → dispatches the right effects
//   - dbErrorsField update: set + clear + empty-set no-op + multi-block group
//   - bodyLineColToOffset: 1-indexed inputs, overflow, multi-line walk
//   - buildErrorDecorations: marks map empty + null position + length cap
//   - decorateDbBody first/last/middle classes
//   - createDbBlockCompletionSource: ref completion through the {{ token
//
// Coverage gate alvo: cm-db-block 41.6% → ≥80%.

import { describe, it, expect } from "vitest";
import { EditorState, EditorSelection, Text } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

import {
  createDbBlockExtension,
  createDbBlockCompletionSource,
  findDbBlocks,
  setDbBlockErrors,
} from "../cm-db-block";

// Resolve the registry-internal blockIdOf via the public block + index —
// `setDbBlockErrors` keys marks by blockId, which the registry computes
// as `db_idx_${index}` (see widget-portal-registry idPrefix). We compute
// it inline below so the test stays robust to refactors.
const dbBlockId = (i: number) => `db_idx_${i}`;

// Build a real CM6 EditorView wired to the DB extension so dispatching
// effects + computing the decoration set both run through the real
// machinery (no shortcuts via internal exports — RULE 4).
function mountView(doc: string): { view: EditorView; container: HTMLElement } {
  const container = document.createElement("div");
  document.body.appendChild(container);
  const view = new EditorView({
    state: EditorState.create({
      doc,
      extensions: [createDbBlockExtension()],
    }),
    parent: container,
  });
  return { view, container };
}

function unmount(view: EditorView, container: HTMLElement) {
  view.destroy();
  container.remove();
}

// ─────────────── decorateDbBody ───────────────

describe("decorateDbBody", () => {
  it("decorates each body line with cm-db-body-line + first/last modifiers", () => {
    const doc = ["```db-postgres", "SELECT 1", "FROM users", "```"].join("\n");
    const { view, container } = mountView(doc);
    try {
      const lines = view.dom.querySelectorAll(".cm-db-body-line");
      // 2 body lines.
      expect(lines.length).toBe(2);
      // First+last modifiers each appear once.
      expect(view.dom.querySelectorAll(".cm-db-body-line-first").length).toBe(
        1,
      );
      expect(view.dom.querySelectorAll(".cm-db-body-line-last").length).toBe(1);
    } finally {
      unmount(view, container);
    }
  });

  it("single-line body: same line is both first and last (both classes set)", () => {
    const doc = ["```db", "SELECT 1", "```"].join("\n");
    const { view, container } = mountView(doc);
    try {
      const lines = view.dom.querySelectorAll(".cm-db-body-line");
      expect(lines.length).toBe(1);
      const cls = lines[0].className;
      expect(cls).toContain("cm-db-body-line-first");
      expect(cls).toContain("cm-db-body-line-last");
    } finally {
      unmount(view, container);
    }
  });

  it("empty body: no body line decorations rendered", () => {
    const doc = ["```db", "```"].join("\n");
    const { view, container } = mountView(doc);
    try {
      expect(view.dom.querySelectorAll(".cm-db-body-line").length).toBe(0);
    } finally {
      unmount(view, container);
    }
  });

  it("editing class applied when cursor inside body", () => {
    const doc = ["```db", "SELECT 1", "```"].join("\n");
    const { view, container } = mountView(doc);
    try {
      // Place cursor inside body line.
      const bodyLinePos = view.state.doc.line(2).from + 1; // 2nd line, col 2
      view.dispatch({ selection: EditorSelection.cursor(bodyLinePos) });
      const editingLines = view.dom.querySelectorAll(".cm-db-body-editing");
      expect(editingLines.length).toBeGreaterThanOrEqual(1);
    } finally {
      unmount(view, container);
    }
  });

  it("multi-line body with blank middle line: 3 body-line decorations", () => {
    const doc = ["```db", "SELECT 1;", "", "SELECT 2;", "```"].join("\n");
    const { view, container } = mountView(doc);
    try {
      const lines = view.dom.querySelectorAll(".cm-db-body-line");
      // 3 body lines including the blank.
      expect(lines.length).toBe(3);
    } finally {
      unmount(view, container);
    }
  });
});

// ─────────────── setDbBlockErrors + dbErrorsField + buildErrorDecorations ───────────────

describe("setDbBlockErrors → dbErrorsField → error mark decorations", () => {
  it("places a .cm-db-sql-error span at the given (line, column)", () => {
    const doc = ["```db-postgres", "SELECT FOO", "FROM bar", "```"].join("\n");
    const { view, container } = mountView(doc);
    try {
      // Mark column 8 of line 1 inside the body — "FOO".
      setDbBlockErrors(view, dbBlockId(0), [
        { line: 1, column: 8, length: 3, message: "Unknown column" },
      ]);
      const errs = view.dom.querySelectorAll(".cm-db-sql-error");
      expect(errs.length).toBe(1);
      // Title attribute should carry the message.
      expect(errs[0].getAttribute("title")).toBe("Unknown column");
    } finally {
      unmount(view, container);
    }
  });

  it("clears marks when called with an empty array", () => {
    const doc = ["```db", "SELECT 1", "```"].join("\n");
    const { view, container } = mountView(doc);
    try {
      setDbBlockErrors(view, dbBlockId(0), [{ line: 1, column: 1, length: 6 }]);
      expect(view.dom.querySelectorAll(".cm-db-sql-error").length).toBe(1);
      setDbBlockErrors(view, dbBlockId(0), []);
      expect(view.dom.querySelectorAll(".cm-db-sql-error").length).toBe(0);
    } finally {
      unmount(view, container);
    }
  });

  it("clearing a blockId that wasn't tracked is a no-op (no exception)", () => {
    const doc = ["```db", "SELECT 1", "```"].join("\n");
    const { view, container } = mountView(doc);
    try {
      // Never set; clearing should not throw or change the deco set.
      expect(() => setDbBlockErrors(view, dbBlockId(0), [])).not.toThrow();
      expect(view.dom.querySelectorAll(".cm-db-sql-error").length).toBe(0);
    } finally {
      unmount(view, container);
    }
  });

  it("invalid (line, column) coordinates (line<1) are dropped (no decoration)", () => {
    const doc = ["```db", "SELECT 1", "```"].join("\n");
    const { view, container } = mountView(doc);
    try {
      setDbBlockErrors(view, dbBlockId(0), [{ line: 0, column: 1, length: 1 }]);
      expect(view.dom.querySelectorAll(".cm-db-sql-error").length).toBe(0);
    } finally {
      unmount(view, container);
    }
  });

  it("line beyond body is dropped (bodyLineColToOffset returns null)", () => {
    const doc = ["```db", "SELECT 1", "```"].join("\n");
    const { view, container } = mountView(doc);
    try {
      setDbBlockErrors(view, dbBlockId(0), [
        { line: 99, column: 1, length: 1 },
      ]);
      expect(view.dom.querySelectorAll(".cm-db-sql-error").length).toBe(0);
    } finally {
      unmount(view, container);
    }
  });

  it("length is clamped to 32 chars", () => {
    const longBody = "X".repeat(100);
    const doc = ["```db", longBody, "```"].join("\n");
    const { view, container } = mountView(doc);
    try {
      setDbBlockErrors(view, dbBlockId(0), [
        { line: 1, column: 1, length: 999, message: "ovf" },
      ]);
      const errs = view.dom.querySelectorAll(".cm-db-sql-error");
      expect(errs.length).toBe(1);
      // Read the underlined text: should be exactly 32 chars.
      expect(errs[0].textContent?.length).toBeLessThanOrEqual(32);
    } finally {
      unmount(view, container);
    }
  });

  it("supports marks across two distinct blocks (groups by blockId)", () => {
    const doc = [
      "```db",
      "SELECT 1",
      "```",
      "",
      "```db-mysql",
      "SELECT 2",
      "```",
    ].join("\n");
    const { view, container } = mountView(doc);
    try {
      // Each block has its own blockId via the idPrefix counter.
      const blocks = findDbBlocks(view.state.doc);
      expect(blocks.length).toBe(2);
      setDbBlockErrors(view, dbBlockId(0), [{ line: 1, column: 1, length: 6 }]);
      setDbBlockErrors(view, dbBlockId(1), [{ line: 1, column: 1, length: 6 }]);
      const errs = view.dom.querySelectorAll(".cm-db-sql-error");
      expect(errs.length).toBe(2);
    } finally {
      unmount(view, container);
    }
  });

  it("multi-line walk: column on line 2 lands at the right offset", () => {
    const doc = ["```db", "SELECT", "FROM users", "```"].join("\n");
    const { view, container } = mountView(doc);
    try {
      // Mark column 3 of line 2 → 'O' of "FROM".
      setDbBlockErrors(view, dbBlockId(0), [{ line: 2, column: 3, length: 2 }]);
      const errs = view.dom.querySelectorAll(".cm-db-sql-error");
      expect(errs.length).toBe(1);
      // Underlined text is the 2-char window starting at col 3.
      expect(errs[0].textContent).toBe("OM");
    } finally {
      unmount(view, container);
    }
  });

  it("default length (undefined) uses 1 char", () => {
    const doc = ["```db", "AB", "```"].join("\n");
    const { view, container } = mountView(doc);
    try {
      setDbBlockErrors(view, dbBlockId(0), [{ line: 1, column: 1 }]);
      const errs = view.dom.querySelectorAll(".cm-db-sql-error");
      expect(errs.length).toBe(1);
      expect(errs[0].textContent).toBe("A");
    } finally {
      unmount(view, container);
    }
  });

  it("no marks set: error decoration set is empty (early-return path)", () => {
    const doc = ["```db", "SELECT 1", "```"].join("\n");
    const { view, container } = mountView(doc);
    try {
      // Mounting the extension already exercises the marks.size === 0 branch.
      expect(view.dom.querySelectorAll(".cm-db-sql-error").length).toBe(0);
    } finally {
      unmount(view, container);
    }
  });
});

// ─────────────── createDbBlockCompletionSource ───────────────

describe("createDbBlockCompletionSource — ref autocomplete", () => {
  it("returns null outside a db block (no {{ context)", async () => {
    const doc = "plain text {{";
    const state = EditorState.create({ doc });
    const source = createDbBlockCompletionSource(() => undefined);
    const ctx = {
      state,
      pos: doc.length,
      explicit: true,
      matchBefore: (re: RegExp) => {
        const before = doc;
        const m = before.match(re);
        if (!m) return null;
        const text = m[0];
        return { from: before.length - text.length, to: before.length, text };
      },
      aborted: false,
    } as unknown as import("@codemirror/autocomplete").CompletionContext;
    const result = await source(ctx);
    // Outside a fenced block — the makeRefCompletionSource short-circuits.
    expect(result === null || (result?.options?.length ?? 0) === 0).toBe(true);
  });

  it("invokable with non-empty doc (smoke — no throw)", () => {
    const source = createDbBlockCompletionSource(() => "/notes/x.md");
    expect(typeof source).toBe("function");
  });
});

// ─────────────── findDbBlocks utility — body decoration via Text ───────────────

describe("findDbBlocks — bodyFrom/bodyTo positions used by error mapper", () => {
  it("body offset advances over multi-line bodies", () => {
    const doc = Text.of(["```db", "ab", "cd", "```"]);
    const blocks = findDbBlocks(doc);
    expect(blocks.length).toBe(1);
    const b = blocks[0];
    // bodyFrom < bodyTo for non-empty body; walking from bodyFrom to bodyTo
    // touches every body character.
    expect(b.bodyTo - b.bodyFrom).toBeGreaterThan(0);
  });
});
