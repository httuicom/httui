// Canvas §6 Variables — page composition.
//
// Three-column layout 200/1fr/380. Owns scope selection + search
// state locally. Slice 2 lights up the rows path: pass `rows` and the
// page derives (scope filter + search match + name sort) and renders
// `<VariableListRow>` for each. `rowsSlot` still wins when a consumer
// wants full custom composition.

import { Flex } from "@chakra-ui/react";
import { useMemo, useState, type ReactNode } from "react";

import { VariableListRow } from "./VariableListRow";
import { VariablesDetailPanel } from "./VariablesDetailPanel";
import { VariablesListPanel } from "./VariablesListPanel";
import { VariablesScopesSidebar } from "./VariablesScopesSidebar";
import {
  countVariableScopes,
  deriveVariableRows,
  type VariableRow,
} from "./variable-derive";
import { VARIABLE_SCOPES, type VariableScope } from "./variable-scopes";

export interface VariablesPageProps {
  /** Initial selected scope. Defaults to "all". */
  initialScope?: VariableScope;
  /** Env names rendered as table column headers (canvas: local/staging/prod). */
  envColumnNames?: ReadonlyArray<string>;
  /** Active env name shown in the right-of-search pill. */
  activeEnvName?: string;
  /** Per-scope counts (overrides the derive(rows) fallback). */
  countsByScope?: Partial<Record<VariableScope, number>>;
  /** Raw rows. When provided AND `rowsSlot` is absent, the page
   * derives (filter+search+sort) and renders rows itself. */
  rows?: ReadonlyArray<VariableRow>;
  selectedKey?: string | null;
  onSelectKey?: (key: string) => void;
  onImportDotenv?: () => void;
  onCreateNew?: () => void;
  /** Manual composition slot. Wins over the auto-rows path. */
  rowsSlot?: ReactNode;
  detailSlot?: ReactNode;
  /** Slot for `<NewVariableForm>` rendered above the rows. */
  inlineFormSlot?: ReactNode;
}

export function VariablesPage({
  initialScope = "all",
  envColumnNames = [],
  activeEnvName,
  countsByScope,
  rows,
  selectedKey,
  onSelectKey,
  onImportDotenv,
  onCreateNew,
  rowsSlot,
  detailSlot,
  inlineFormSlot,
}: VariablesPageProps) {
  const [scope, setScope] = useState<VariableScope>(
    VARIABLE_SCOPES.includes(initialScope) ? initialScope : "all",
  );
  const [search, setSearch] = useState("");

  const derivedRows = useMemo(
    () => (rows ? deriveVariableRows({ rows, scope, search }) : undefined),
    [rows, scope, search],
  );

  const counts = useMemo(() => {
    if (countsByScope) return countsByScope;
    if (rows) return countVariableScopes(rows);
    return undefined;
  }, [countsByScope, rows]);

  const autoRows: ReactNode = derivedRows
    ? derivedRows.map((row) => (
        <VariableListRow
          key={row.key}
          row={row}
          envColumnNames={envColumnNames}
          selected={row.key === selectedKey}
          onClick={() => onSelectKey?.(row.key)}
        />
      ))
    : null;

  const finalRowsSlot =
    rowsSlot !== undefined
      ? rowsSlot
      : derivedRows && derivedRows.length > 0
        ? autoRows
        : undefined;

  return (
    <Flex
      data-testid="variables-page"
      data-scope={scope}
      h="full"
      minH={0}
      overflow="hidden"
    >
      <VariablesScopesSidebar
        selectedScope={scope}
        onSelectScope={setScope}
        countsByScope={counts}
      />
      <VariablesListPanel
        envColumnNames={envColumnNames}
        activeEnvName={activeEnvName}
        searchValue={search}
        onSearchChange={setSearch}
        onImportDotenv={onImportDotenv}
        onCreateNew={onCreateNew}
        rowsSlot={finalRowsSlot}
        inlineFormSlot={inlineFormSlot}
      />
      <VariablesDetailPanel selectedKey={selectedKey}>
        {detailSlot}
      </VariablesDetailPanel>
    </Flex>
  );
}
