import { describe, it, expect, afterEach } from "vitest";
import { EditorState, Text } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

import {
  tables,
  findTables,
  formatTable,
  findTableAtBoundary,
} from "@/lib/codemirror/cm-tables";

// `| h1 | h2 |` header + separator + one row — the minimal renderable table.
const TABLE = "| h1 | h2 |\n| --- | --- |\n| a | b |";
const doc = (s: string) => Text.of(s.split("\n"));

describe("findTables", () => {
  it("finds a header + separator + body rows", () => {
    const t = findTables(doc(TABLE));
    expect(t).toHaveLength(1);
    expect(t[0].rows).toEqual([
      ["h1", "h2"],
      ["a", "b"],
    ]);
    expect(t[0].hasHeader).toBe(true);
  });

  it("ignores a pipe row with no separator beneath it", () => {
    expect(findTables(doc("| a | b |\nplain text"))).toEqual([]);
  });

  it("ignores a non-pipe line", () => {
    expect(findTables(doc("# title\n\nbody"))).toEqual([]);
  });

  it("collects multiple tables and stops body at the first non-row", () => {
    const two = `${TABLE}\n\nmid\n\n${TABLE}`;
    const t = findTables(doc(two));
    expect(t).toHaveLength(2);
  });

  it("handles a table at end of document", () => {
    const t = findTables(doc(`intro\n${TABLE}`));
    expect(t).toHaveLength(1);
    expect(t[0].rows).toHaveLength(2);
  });
});

describe("formatTable", () => {
  it("pads columns to align a misaligned table", () => {
    const d = doc("| a | bbbbb |\n| --- | --- |\n| ccccc | d |");
    const out = formatTable(d, findTables(d)[0]);
    expect(out).not.toBeNull();
    // every body/header row is padded to the same width
    const widths = out!.split("\n").map((l) => l.length);
    expect(new Set(widths).size).toBe(1);
  });

  it("returns null when the table is already aligned", () => {
    const aligned = "| h1  | h2  |\n| --- | --- |\n| a   | b   |";
    const d = doc(aligned);
    expect(formatTable(d, findTables(d)[0])).toBeNull();
  });
});

describe("findTableAtBoundary", () => {
  const d = doc(`intro\n${TABLE}\nafter`);

  it("matches a table whose first line is the queried line going down", () => {
    // table header is line 2 (1-based)
    expect(findTableAtBoundary(d, 2, "down")).not.toBeNull();
    expect(findTableAtBoundary(d, 3, "down")).toBeNull();
  });

  it("matches a table whose last line is the queried line going up", () => {
    // table last row is line 4
    expect(findTableAtBoundary(d, 4, "up")).not.toBeNull();
    expect(findTableAtBoundary(d, 2, "up")).toBeNull();
  });
});

describe("tables extension", () => {
  let view: EditorView;
  afterEach(() => view?.destroy());

  // anchor keeps the cursor off the table so it renders as a widget
  // (cursor inside a table shows the raw markdown instead)
  function mount(text: string, anchor = 0) {
    view = new EditorView({
      state: EditorState.create({
        doc: text,
        selection: { anchor },
        extensions: [tables()],
      }),
      parent: document.body,
    });
    return view;
  }

  it("renders a table widget with header + body cells when cursor is outside", () => {
    mount("intro\n\n" + TABLE);
    const table = view.dom.querySelector(".cm-table-widget");
    expect(table).toBeTruthy();
    expect(table!.querySelectorAll("thead th")).toHaveLength(2);
    expect(table!.querySelectorAll("tbody td")).toHaveLength(2);
  });

  it("shows raw lines when the cursor is inside the table", () => {
    // place the cursor inside the table body
    mount("intro\n\n" + TABLE, "intro\n\n".length + TABLE.length - 2);
    expect(view.dom.querySelector(".cm-table-raw")).toBeTruthy();
    expect(view.dom.querySelector(".cm-table-widget")).toBeNull();
  });

  it("renders two widgets for a doc with two tables", () => {
    mount(`intro\n\n${TABLE}\n\nmid\n\n${TABLE}`);
    expect(view.dom.querySelectorAll(".cm-table-widget")).toHaveLength(2);
  });

  // The docChanged early-out skips the full rescan when no table is
  // present and the edit inserts no `|`; these assert it stays correct.
  it("stays plain when editing prose with no pipe char", () => {
    mount("# title\n\nbody text\nmore lines\n");
    view.dispatch({
      changes: { from: view.state.doc.length, insert: " word" },
    });
    expect(view.dom.querySelector(".cm-table-widget")).toBeNull();
  });

  it("renders a table pasted into a clean doc", () => {
    mount("# title\n\nbody\n"); // cursor stays at 0, outside the table
    view.dispatch({
      changes: { from: view.state.doc.length, insert: "\n" + TABLE },
    });
    expect(view.dom.querySelector(".cm-table-widget")).toBeTruthy();
  });

  it("keeps the table after an unrelated prose edit", () => {
    mount("intro\n\n" + TABLE);
    view.dispatch({ changes: { from: 0, insert: "x" } });
    expect(view.dom.querySelector(".cm-table-widget")).toBeTruthy();
  });
});
