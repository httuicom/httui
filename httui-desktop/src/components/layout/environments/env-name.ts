// Canvas §6 Environments — env-name validation (Epic 44 Story 02).
//
// Pure validator for the create/clone inline forms. Env names become
// filenames (`envs/<name>.toml` or `envs/<name>.local.toml`), so the
// rejection set is stricter than variable names: no whitespace, no
// slash / backslash, no leading dot, no trailing `.toml` (we add the
// suffix), no case-insensitive duplicate against existing names.
// Existing names are compared without their `.toml` / `.local.toml`
// suffix so `staging` collides with both `staging.toml` and
// `staging.local.toml`.

import { envNameFromFilename } from "./envs-meta";

export type EnvNameValidation = { ok: true } | { ok: false; reason: string };

export function validateEnvName(
  name: string,
  existingFilenames: ReadonlyArray<string> = [],
): EnvNameValidation {
  const trimmed = name.trim();
  if (!trimmed) {
    return { ok: false, reason: "Name is required" };
  }
  if (/\s/.test(trimmed)) {
    return { ok: false, reason: "Cannot contain whitespace" };
  }
  if (trimmed.includes("/") || trimmed.includes("\\")) {
    return { ok: false, reason: "Cannot contain / or \\" };
  }
  if (trimmed.startsWith(".")) {
    return { ok: false, reason: "Cannot start with a dot" };
  }
  if (trimmed.toLowerCase().endsWith(".toml")) {
    return { ok: false, reason: "Drop .toml — added automatically" };
  }
  const lower = trimmed.toLowerCase();
  for (const filename of existingFilenames) {
    if (envNameFromFilename(filename).trim().toLowerCase() === lower) {
      return {
        ok: false,
        reason: "An environment with this name already exists",
      };
    }
  }
  return { ok: true };
}
