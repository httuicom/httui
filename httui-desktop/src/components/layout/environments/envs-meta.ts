const LOCAL_SUFFIX = ".local.toml";
const TOML_SUFFIX = ".toml";

export interface EnvironmentSummary {
  /** Display name (filename minus the `.toml` / `.local.toml` suffix). */
  name: string;
  /** Original filename relative to `envs/`. Used as identity. */
  filename: string;
  /** Number of variables defined in this env. */
  varCount: number;
  /** Number of connections referenced by `[meta].connections_used`. 0 = all. */
  connectionsUsedCount: number;
  /** True when this is the workspace's current active env. */
  isActive: boolean;
  /** True when the file ends with `.local.toml` (gitignored). */
  isPersonal: boolean;
  /** True when `[meta].temporary = true` in the TOML. */
  isTemporary: boolean;
  /** Optional `[meta].description` value. */
  description?: string;
}

export function isPersonalEnvFilename(filename: string): boolean {
  return filename.endsWith(LOCAL_SUFFIX);
}

export function envNameFromFilename(filename: string): string {
  if (filename.endsWith(LOCAL_SUFFIX)) {
    return filename.slice(0, -LOCAL_SUFFIX.length);
  }
  if (filename.endsWith(TOML_SUFFIX)) {
    return filename.slice(0, -TOML_SUFFIX.length);
  }
  return filename;
}

/** Sort alpha (case-insensitive). Active env is not pinned to position 0 —
 * stable card positions let the FLIP animation slide the ACTIVE pill smoothly. */
export function sortEnvironments(
  envs: ReadonlyArray<EnvironmentSummary>,
): ReadonlyArray<EnvironmentSummary> {
  return [...envs].sort((a, b) =>
    a.name.localeCompare(b.name, undefined, { sensitivity: "base" }),
  );
}
