// Canvas §6 Variables — derivation helpers.
//
// Pure functions: scope filter + search match + name sort, plus a
// composed `deriveVariableRows` and a `countVariableScopes` mirror that
// powers the sidebar counts when the consumer doesn't pass them.

import type { VariableScope } from "./variable-scopes";

/** Discriminator for a variable's home; "secret" lives separately as
 * a cross-cutting `isSecret` flag (see scope filter below). */
export type VariableScopeKind = "workspace" | "captured" | "personal";

export interface VariableRow {
  key: string;
  scope: VariableScopeKind;
  /** True when the value lives in keychain — masks at the row level
   * and pulls the row into the "secret" sidebar view. */
  isSecret: boolean;
  /** Value per env id (undefined → render em-dash). */
  values: Readonly<Record<string, string | undefined>>;
  /** Number of `{{KEY}}` references found across the vault. */
  usesCount: number;
}

export function applyVariableScope(
  rows: ReadonlyArray<VariableRow>,
  scope: VariableScope,
): ReadonlyArray<VariableRow> {
  switch (scope) {
    case "all":
      return rows;
    case "secret":
      return rows.filter((r) => r.isSecret);
    case "workspace":
    case "captured":
    case "personal":
      return rows.filter((r) => r.scope === scope);
  }
}

export function matchVariableSearch(row: VariableRow, search: string): boolean {
  const needle = search.trim().toLowerCase();
  if (!needle) return true;
  if (row.key.toLowerCase().includes(needle)) return true;
  if (row.scope.toLowerCase().includes(needle)) return true;
  for (const value of Object.values(row.values)) {
    if (value && value.toLowerCase().includes(needle)) return true;
  }
  return false;
}

export function sortVariableRowsByName(
  rows: ReadonlyArray<VariableRow>,
): ReadonlyArray<VariableRow> {
  return [...rows].sort((a, b) =>
    a.key.localeCompare(b.key, undefined, { sensitivity: "base" }),
  );
}

export interface DeriveVariableRowsArgs {
  rows: ReadonlyArray<VariableRow>;
  scope: VariableScope;
  search: string;
}

export function deriveVariableRows({
  rows,
  scope,
  search,
}: DeriveVariableRowsArgs): ReadonlyArray<VariableRow> {
  const scoped = applyVariableScope(rows, scope);
  const matched = scoped.filter((r) => matchVariableSearch(r, search));
  return sortVariableRowsByName(matched);
}

export function countVariableScopes(
  rows: ReadonlyArray<VariableRow>,
): Record<VariableScope, number> {
  const counts: Record<VariableScope, number> = {
    all: rows.length,
    workspace: 0,
    captured: 0,
    secret: 0,
    personal: 0,
  };
  for (const row of rows) {
    counts[row.scope] += 1;
    if (row.isSecret) counts.secret += 1;
  }
  return counts;
}
