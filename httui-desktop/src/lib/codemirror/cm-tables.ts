import {
  RangeSetBuilder,
  StateField,
  type Extension,
  type EditorState,
  EditorSelection,
  Prec,
} from "@codemirror/state";
import {
  Decoration,
  type DecorationSet,
  EditorView,
  WidgetType,
  keymap,
} from "@codemirror/view";

// ── Table detection ──────────────────────────────────────────────────────────

interface TableRange {
  from: number;
  to: number;
  rows: string[][];
  hasHeader: boolean;
}

const PIPE_ROW_RE = /^\|(.+)\|$/;
const SEPARATOR_RE = /^\|[\s:]*-{2,}[\s:]*(\|[\s:]*-{2,}[\s:]*)*\|$/;

function parseRow(line: string): string[] | null {
  const match = line.trim().match(PIPE_ROW_RE);
  if (!match) return null;
  return match[1].split("|").map((cell) => cell.trim());
}

function findTables(doc: {
  lines: number;
  line(n: number): { from: number; to: number; text: string };
}): TableRange[] {
  const tables: TableRange[] = [];
  let i = 1;

  while (i <= doc.lines) {
    const line = doc.line(i);
    const headerCells = parseRow(line.text);

    if (headerCells && i + 1 <= doc.lines) {
      const sepLine = doc.line(i + 1);
      if (SEPARATOR_RE.test(sepLine.text.trim())) {
        const rows: string[][] = [headerCells];
        const tableFrom = line.from;
        let tableTo = sepLine.to;
        let j = i + 2;

        while (j <= doc.lines) {
          const bodyLine = doc.line(j);
          const bodyCells = parseRow(bodyLine.text);
          if (!bodyCells) break;
          rows.push(bodyCells);
          tableTo = bodyLine.to;
          j++;
        }

        tables.push({ from: tableFrom, to: tableTo, rows, hasHeader: true });
        i = j;
        continue;
      }
    }
    i++;
  }

  return tables;
}

// ── Table formatting (align columns with spaces) ────────────────────────────

function formatTable(
  doc: EditorState["doc"],
  table: TableRange,
): string | null {
  const startLine = doc.lineAt(table.from).number;
  const endLine = doc.lineAt(table.to).number;

  // Parse all rows (including separator)
  const allLines: string[] = [];
  const parsedRows: (string[] | "separator")[] = [];

  for (let i = startLine; i <= endLine; i++) {
    const line = doc.line(i);
    allLines.push(line.text);
    if (SEPARATOR_RE.test(line.text.trim())) {
      parsedRows.push("separator");
    } else {
      const cells = parseRow(line.text);
      parsedRows.push(cells ?? []);
    }
  }

  // Compute max width per column
  const colCount = Math.max(
    ...parsedRows
      .filter((r): r is string[] => r !== "separator")
      .map((r) => r.length),
  );
  if (colCount === 0) return null;

  const colWidths: number[] = new Array(colCount).fill(3); // min 3 for ---
  for (const row of parsedRows) {
    if (row === "separator") continue;
    for (let c = 0; c < row.length; c++) {
      colWidths[c] = Math.max(colWidths[c], row[c].length);
    }
  }

  // Build formatted lines
  const formatted: string[] = [];
  for (const row of parsedRows) {
    if (row === "separator") {
      const sep = colWidths.map((w) => "-".repeat(w)).join(" | ");
      formatted.push(`| ${sep} |`);
    } else {
      const cells = colWidths.map((w, c) => {
        const cell = row[c] ?? "";
        return cell.padEnd(w);
      });
      formatted.push(`| ${cells.join(" | ")} |`);
    }
  }

  const result = formatted.join("\n");
  const original = allLines.join("\n");
  return result !== original ? result : null;
}

// ── Table widget ─────────────────────────────────────────────────────────────

class TableWidget extends WidgetType {
  constructor(
    readonly rows: string[][],
    readonly hasHeader: boolean,
  ) {
    super();
  }

  toDOM(): HTMLElement {
    const table = document.createElement("table");
    table.className = "cm-table-widget";

    if (this.hasHeader && this.rows.length > 0) {
      const thead = document.createElement("thead");
      const headerRow = document.createElement("tr");
      for (const cell of this.rows[0]) {
        const th = document.createElement("th");
        th.textContent = cell;
        headerRow.appendChild(th);
      }
      thead.appendChild(headerRow);
      table.appendChild(thead);

      if (this.rows.length > 1) {
        const tbody = document.createElement("tbody");
        for (let i = 1; i < this.rows.length; i++) {
          const tr = document.createElement("tr");
          for (const cell of this.rows[i]) {
            const td = document.createElement("td");
            td.textContent = cell;
            tr.appendChild(td);
          }
          tbody.appendChild(tr);
        }
        table.appendChild(tbody);
      }
    }

    return table;
  }

  eq(other: TableWidget): boolean {
    if (this.rows.length !== other.rows.length) return false;
    for (let i = 0; i < this.rows.length; i++) {
      if (this.rows[i].length !== other.rows[i].length) return false;
      for (let j = 0; j < this.rows[i].length; j++) {
        if (this.rows[i][j] !== other.rows[i][j]) return false;
      }
    }
    return true;
  }

  ignoreEvent(): boolean {
    return true;
  }
}

// ── Decoration builder ──────────────────────────────────────────────────────

// Track which table the cursor was previously inside (for format-on-exit)
let prevCursorTableIdx = -1;

function buildTableDecorations(
  state: EditorState,
  view?: EditorView,
): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>();

  const cursorLines = new Set<number>();
  for (const range of state.selection.ranges) {
    const startLine = state.doc.lineAt(range.from).number;
    const endLine = state.doc.lineAt(range.to).number;
    for (let i = startLine; i <= endLine; i++) {
      cursorLines.add(i);
    }
  }

  const tables = findTables(state.doc);

  // Find which table cursor is currently in
  let currentTableIdx = -1;

  for (let t = 0; t < tables.length; t++) {
    const table = tables[t];
    const tableStartLine = state.doc.lineAt(table.from).number;
    const tableEndLine = state.doc.lineAt(table.to).number;

    for (let line = tableStartLine; line <= tableEndLine; line++) {
      if (cursorLines.has(line)) {
        currentTableIdx = t;
        break;
      }
    }
  }

  // Format-on-exit: if cursor left a table, format it
  if (
    prevCursorTableIdx !== -1 &&
    currentTableIdx !== prevCursorTableIdx &&
    view
  ) {
    const prevTable = tables[prevCursorTableIdx];
    if (prevTable) {
      const formatted = formatTable(state.doc, prevTable);
      if (formatted) {
        // Schedule the format dispatch after this update completes
        queueMicrotask(() => {
          view.dispatch({
            changes: {
              from: prevTable.from,
              to: prevTable.to,
              insert: formatted,
            },
          });
        });
      }
    }
  }
  prevCursorTableIdx = currentTableIdx;

  for (let t = 0; t < tables.length; t++) {
    const table = tables[t];
    const isCursorIn = t === currentTableIdx;

    if (isCursorIn) {
      // Cursor inside — show raw pipes (mono font only)
      const tableStartLine = state.doc.lineAt(table.from).number;
      const tableEndLine = state.doc.lineAt(table.to).number;
      for (let line = tableStartLine; line <= tableEndLine; line++) {
        const lineObj = state.doc.line(line);
        builder.add(
          lineObj.from,
          lineObj.from,
          Decoration.line({ class: "cm-table-raw" }),
        );
      }
      continue;
    }

    // Cursor outside — render as HTML table widget
    builder.add(
      table.from,
      table.to,
      Decoration.replace({
        widget: new TableWidget(table.rows, table.hasHeader),
        block: true,
      }),
    );
  }

  return builder.finish();
}

let lastTableCursorLine = -1;

const tableField = StateField.define<DecorationSet>({
  create(state) {
    lastTableCursorLine = state.doc.lineAt(state.selection.main.head).number;
    return buildTableDecorations(state);
  },
  update(decos, tr) {
    const currentLine = tr.state.doc.lineAt(
      tr.state.selection.main.head,
    ).number;
    const cursorLineMoved = currentLine !== lastTableCursorLine;

    if (cursorLineMoved) {
      lastTableCursorLine = currentLine;
      // Pass the view for format-on-exit
      const view = (tr as unknown as { view?: EditorView }).view;
      return buildTableDecorations(tr.state, view);
    }
    if (tr.docChanged) {
      return buildTableDecorations(tr.state);
    }
    return decos;
  },
  provide: (f) => EditorView.decorations.from(f),
});

// ── Arrow key navigation into tables ─────────────────────────────────────────

function findTableAtBoundary(
  doc: EditorState["doc"],
  lineNum: number,
  direction: "down" | "up",
): TableRange | null {
  const tables = findTables(doc);
  for (const table of tables) {
    const startLine = doc.lineAt(table.from).number;
    const endLine = doc.lineAt(table.to).number;
    if (direction === "down" && startLine === lineNum) return table;
    if (direction === "up" && endLine === lineNum) return table;
  }
  return null;
}

const tableKeymap = keymap.of([
  {
    key: "ArrowDown",
    run(view) {
      const { state } = view;
      const cursorLine = state.doc.lineAt(state.selection.main.head).number;
      const nextLine = cursorLine + 1;
      if (nextLine > state.doc.lines) return false;
      const table = findTableAtBoundary(state.doc, nextLine, "down");
      if (!table) return false;
      const targetLine = state.doc.line(nextLine);
      view.dispatch({ selection: EditorSelection.cursor(targetLine.from) });
      return true;
    },
  },
  {
    key: "ArrowUp",
    run(view) {
      const { state } = view;
      const cursorLine = state.doc.lineAt(state.selection.main.head).number;
      const prevLine = cursorLine - 1;
      if (prevLine < 1) return false;
      const table = findTableAtBoundary(state.doc, prevLine, "up");
      if (!table) return false;
      const targetLine = state.doc.line(prevLine);
      view.dispatch({ selection: EditorSelection.cursor(targetLine.to) });
      return true;
    },
  },
]);

// ── Theme ────────────────────────────────────────────────────────────────────

const tableTheme = EditorView.theme({
  // Rendered table widget (cursor outside)
  ".cm-table-widget": {
    borderCollapse: "collapse",
    width: "100%",
    margin: "4px 0",
    fontSize: "13px",
    fontFamily: "var(--chakra-fonts-body)",
  },
  ".cm-table-widget th, .cm-table-widget td": {
    border: "1px solid var(--chakra-colors-border)",
    padding: "8px 12px",
    textAlign: "left",
  },
  ".cm-table-widget th": {
    fontWeight: "600",
    backgroundColor: "var(--chakra-colors-bg-subtle)",
  },
  ".cm-table-widget tr:hover td": {
    backgroundColor: "var(--chakra-colors-bg-subtle)",
  },
  // Raw table lines (cursor inside) — just mono font, no extra decoration
  ".cm-table-raw": {
    fontFamily: "var(--chakra-fonts-mono)",
    fontSize: "13px",
  },
});

// ── Export ────────────────────────────────────────────────────────────────────

/** GFM table extension — renders pipe tables as HTML widgets, raw when cursor is inside */
export function tables(): Extension {
  return [tableField, tableTheme, Prec.high(tableKeymap)];
}
