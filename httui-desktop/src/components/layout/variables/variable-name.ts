// Pure validator for the new-variable form. Rejects whitespace, `.`
// (dot is the `{{alias.foo.bar}}` path separator), and case-insensitive
// duplicates (upper-snake names like API_BASE must not collide with api_base).

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
