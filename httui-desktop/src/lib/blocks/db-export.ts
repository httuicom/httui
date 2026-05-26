/**
 * Pure serializers for a db select result. Used by the toolbar ⤓ menu.
 * Kept UI-agnostic so they can be tested in isolation.
 *
 * Input is a single `SelectResult` (columns + rows). Mutation / error
 * results don't have tabular output to serialize — the menu is disabled
 * for those.
 */

import type {
  CellValue,
  DbColumn,
  DbResult,
  DbRow,
} from "@/components/blocks/db/types";

type SelectResult = Extract<DbResult, { kind: "select" }>;

// ───── CSV ─────

/**
 * RFC 4180-ish CSV. Quotes fields that contain CRLF, commas, or double
 * quotes. Nulls become empty fields. Complex values (objects/arrays)
 * are emitted as JSON.
 */
export function toCsv(result: SelectResult): string {
  const rows: string[] = [];
  rows.push(result.columns.map((c) => csvEscape(c.name)).join(","));
  for (const row of result.rows) {
    rows.push(
      result.columns
        .map((c) => csvEscape(formatCellCsv(row[c.name])))
        .join(","),
    );
  }
  return rows.join("\n") + "\n";
}

function csvEscape(value: string): string {
  if (/[",\r\n]/.test(value)) {
    return `"${value.replace(/"/g, '""')}"`;
  }
  return value;
}

function formatCellCsv(value: CellValue | undefined): string {
  if (value === null || value === undefined) return "";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return JSON.stringify(value);
}

// ───── JSON ─────

export function toJson(result: SelectResult): string {
  return JSON.stringify(result.rows, null, 2) + "\n";
}

// ───── Markdown ─────

/**
 * GitHub-flavored Markdown table. Pipes and backslashes inside cells are
 * escaped so the table stays syntactically valid.
 */
export function toMarkdown(result: SelectResult): string {
  const header = `| ${result.columns.map((c) => mdEscape(c.name)).join(" | ")} |`;
  const separator = `| ${result.columns.map(() => "---").join(" | ")} |`;
  const body = result.rows.map(
    (row) =>
      `| ${result.columns.map((c) => mdEscape(formatCellMd(row[c.name]))).join(" | ")} |`,
  );
  return [header, separator, ...body].join("\n") + "\n";
}

function mdEscape(value: string): string {
  return value
    .replace(/\\/g, "\\\\")
    .replace(/\|/g, "\\|")
    .replace(/\r?\n/g, " ");
}

function formatCellMd(value: CellValue | undefined): string {
  if (value === null || value === undefined) return "";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return JSON.stringify(value);
}

// ───── INSERT statements ─────

/**
 * Emit one `INSERT INTO <table> (cols) VALUES (...)` per row. Strings get
 * SQL-quoted (single quotes, double-single escaping). Objects/arrays are
 * JSON'd and quoted. Nulls become the literal `NULL`.
 *
 * `tableName` falls back to `"<table>"` when the caller can't infer it
 * from the source query — the user can find-replace it afterwards.
 */
export function toInserts(result: SelectResult, tableName: string): string {
  const safeTable = tableName || "<table>";
  const cols = result.columns.map((c) => identOrQuote(c.name)).join(", ");
  return (
    result.rows
      .map((row) => {
        const values = result.columns
          .map((c) => sqlLiteral(row[c.name]))
          .join(", ");
        return `INSERT INTO ${safeTable} (${cols}) VALUES (${values});`;
      })
      .join("\n") + "\n"
  );
}

/** Quote an identifier only when it contains characters that'd need it. */
function identOrQuote(name: string): string {
  if (/^[A-Za-z_][A-Za-z0-9_]*$/.test(name)) return name;
  return `"${name.replace(/"/g, '""')}"`;
}

function sqlLiteral(value: CellValue | undefined): string {
  if (value === null || value === undefined) return "NULL";
  if (typeof value === "number")
    return Number.isFinite(value) ? String(value) : "NULL";
  if (typeof value === "boolean") return value ? "TRUE" : "FALSE";
  if (typeof value === "string") {
    return `'${value.replace(/'/g, "''")}'`;
  }
  // Array or object → JSON string literal.
  return `'${JSON.stringify(value).replace(/'/g, "''")}'`;
}

// ───── Table name inference ─────

/**
 * Best-effort pull of a table name out of a SQL query: first identifier
 * after the first `FROM` keyword. Used as a sensible default for
 * `INSERT` export. Returns null when it can't find one — the caller
 * falls back to `<table>` in that case.
 */
export function inferTableName(sql: string): string | null {
  const cleaned = sql.replace(/--[^\n]*/g, "").replace(/\/\*[\s\S]*?\*\//g, "");
  const match = cleaned.match(
    /\bFROM\s+([A-Za-z_][\w]*(?:\.[A-Za-z_][\w]*)?)/i,
  );
  return match ? match[1] : null;
}

// ───── Convenience: column header only (used by empty-result detection) ─────

export function hasExportableRows(result: SelectResult): boolean {
  return result.rows.length > 0 && result.columns.length > 0;
}

// Re-export a narrowed type for consumers that don't want to depend on the
// full DbResult union.
export type ExportableResult = SelectResult;

// Silence unused-import warnings for utility types consumed via generics.
export type _ExportInternal = DbColumn & DbRow;
