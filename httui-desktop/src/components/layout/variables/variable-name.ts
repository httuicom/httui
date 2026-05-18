// Canvas §6 Variables — name validation.
//
// Pure validator for the new-variable inline form. Rejects empty,
// whitespace-only, name with internal whitespace, name containing `.`
// (the dot is the reference path separator — `{{alias.foo.bar}}`),
// and duplicates against the supplied existing names. Case-insensitive
// duplicate check because env-files are usually written in upper-snake
// and we don't want a near-collision (`API_BASE` vs `api_base`).

export type NameValidationResult = { ok: true } | { ok: false; reason: string };

export function validateVariableName(
  name: string,
  existing: ReadonlyArray<string> = [],
): NameValidationResult {
  const trimmed = name.trim();
  if (!trimmed) {
    return { ok: false, reason: "Name is required" };
  }
  if (/\s/.test(trimmed)) {
    return { ok: false, reason: "Cannot contain whitespace" };
  }
  if (trimmed.includes(".")) {
    return {
      ok: false,
      reason: "Cannot contain a dot (reference path separator)",
    };
  }
  const lower = trimmed.toLowerCase();
  if (existing.some((n) => n.trim().toLowerCase() === lower)) {
    return { ok: false, reason: "A variable with this name already exists" };
  }
  return { ok: true };
}
