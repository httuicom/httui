
import type { SessionOverrides } from "@/stores/sessionOverride";

import type { VariableRow } from "./variable-derive";

export interface ResolvedVariableValue {
  value: string | undefined;
  isOverridden: boolean;
}

export function resolveVariableValue(
  row: VariableRow,
  env: string,
  overrides: SessionOverrides,
): ResolvedVariableValue {
  const override = overrides[env]?.[row.key];
  if (override !== undefined) {
    return { value: override, isOverridden: true };
  }
  return { value: row.values[env], isOverridden: false };
}

/** True when any env in `envNames` has an override set for `key`. */
export function hasAnyOverride(
  key: string,
  envNames: ReadonlyArray<string>,
  overrides: SessionOverrides,
): boolean {
  for (const env of envNames) {
    if (overrides[env]?.[key] !== undefined) return true;
  }
  return false;
}
