/**
 * Typing-lag benchmark (ADR-010 Slice 2.5).
 *
 * Measures the per-keystroke cost on the UI thread: a single-character
 * edit dispatched into a real EditorView built from `buildExtensions`,
 * over synthetic medium (10 blocks) and large (50 blocks) documents.
 * The dispatch runs every docChanged-triggered StateField/ViewPlugin
 * synchronously — exactly the work a keystroke triggers in the editor.
 *
 * Run: `npm run bench` (vitest bench). Reports hz / mean / p99 per case.
 *
 * jsdom caveat: with no real layout, `view.visibleRanges` covers the
 * whole document, so viewport-scoped plugins (referenceHighlight) scan
 * everything here — a worst-case upper bound, not the browser figure.
 * A browser bench (Playwright) would give the real viewport cost; this
 * jsdom number is the conservative ceiling.
 */
import { bench, describe } from "vitest";
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { buildExtensions } from "../markdown-extensions";

// Deterministic doc synthesis mirroring bench/fixtures from httui-lang:
// alternating HTTP/SQL blocks referencing earlier aliases, padded with
// prose. Kept in-memory so the desktop bench owns no fixture file.
function synthDoc(nBlocks: number, linesPerBlock: number): string {
  const out: string[] = ["# Benchmark fixture", ""];
  for (let b = 0; b < nBlocks; b++) {
    const n = b + 1;
    out.push(`### Block ${n}`, "");
    if (b % 2 === 0) {
      out.push(`\`\`\`http alias=req${n} timeout=30000`);
      out.push(
        n > 1
          ? `GET https://api.example.com/items?parent={{req${n - 2 > 0 ? n - 2 : 1}.response.body.id}}`
          : "GET https://api.example.com/items?page=1",
      );
      out.push(
        "Authorization: Bearer {{TOKEN}}",
        "Content-Type: application/json",
        "",
      );
      out.push('{"page": 1}', "```", "");
    } else {
      out.push(`\`\`\`db-postgres alias=q${n}`);
      out.push(
        `SELECT id, name FROM items WHERE parent = {{req${n - 1}.response.body.id}}`,
      );
      out.push("```", "");
    }
    const prose = [
      "Some prose describing the request and the reference chain below.",
      "",
      "- a bullet describing a field",
      "- another bullet about an edge case",
      "",
    ];
    for (let i = 0; i < linesPerBlock; i++) out.push(prose[i % prose.length]);
  }
  return out.join("\n");
}

function makeView(doc: string): { view: EditorView; editAt: number } {
  const state = EditorState.create({
    doc,
    extensions: buildExtensions({
      filePath: "bench/doc.md",
      entriesRef: { current: [] },
      handleFileSelectRef: { current: () => {} },
      docHeaderHandle: null,
    }),
  });
  const view = new EditorView({ state });
  // Edit point: inside prose near the end of the doc, away from fences,
  // so the change is a plain text insert (the common keystroke).
  const editAt = doc.length - 40;
  return { view, editAt };
}

// medium: 10 blocks / ~500 lines; large: 50 blocks / ~5000 lines.
const medium = synthDoc(10, 38);
const large = synthDoc(50, 90);

describe("typing lag — single keystroke dispatch", () => {
  let mediumView: { view: EditorView; editAt: number };
  let largeView: { view: EditorView; editAt: number };

  bench(
    "medium (10 blocks)",
    () => {
      const { view, editAt } = mediumView;
      // insert then delete so the doc stays stable across iterations
      view.dispatch({ changes: { from: editAt, insert: "x" } });
      view.dispatch({ changes: { from: editAt, to: editAt + 1 } });
    },
    {
      setup: () => {
        mediumView = makeView(medium);
      },
      teardown: () => mediumView.view.destroy(),
    },
  );

  bench(
    "large (50 blocks)",
    () => {
      const { view, editAt } = largeView;
      view.dispatch({ changes: { from: editAt, insert: "x" } });
      view.dispatch({ changes: { from: editAt, to: editAt + 1 } });
    },
    {
      setup: () => {
        largeView = makeView(large);
      },
      teardown: () => largeView.view.destroy(),
    },
  );
});
