// coverage:exclude file — Pure `invoke()` wrappers + IPC type
// declarations. Testing these is testing the mock harness; the real
// behavior lives in the backend. Documented in tech-debt.md and
// audit-002.

import { invoke } from "@tauri-apps/api/core";

// --- Types ---

export interface FileEntry {
  name: string;
  path: string;
  is_dir: boolean;
  children: FileEntry[] | null;
}

// --- Config ---

export function getConfig(key: string): Promise<string | null> {
  return invoke("get_config", { key });
}

export function setConfig(key: string, value: string): Promise<void> {
  return invoke("set_config", { key, value });
}

// --- File-backed vault config (epic 09 foundation) ---
//
// These wrap the new `WorkspaceStore` / `UserStore` surface in
// `httui-core::vault_config`. The frontend keeps using the legacy
// `getConfig`/`setConfig` for prefs until epic 19 cuts the settings
// store over.

/** `[defaults]` section of `<vault>/.httui/workspace.toml`. */
export interface WorkspaceDefaults {
  environment?: string | null;
  git_remote?: string | null;
  git_branch?: string | null;
}

/** `[ui]` section of `~/.config/httui/user.toml`.
 *
 * Theme is serialized as JSON when the user has a richer ThemeConfig
 * than just a mode string; the migration (Story 03) writes a bare
 * mode string, so reads must accept both shapes.
 */
export interface UserUiPrefs {
  theme: string;
  font_family: string;
  font_size: number;
  density: string;
  /** Editor auto-save debounce window (ms). */
  auto_save_ms: number;
  /** DB block default LIMIT when no explicit pin. */
  default_fetch_size: number;
  /** Per-block run-history retention cap. */
  history_retention: number;
  /** Editor vim-mode toggle. */
  vim_enabled: boolean;
  /** Sidebar open/closed flag. */
  sidebar_open: boolean;
  /** Color mode: `"system"` | `"light"` | `"dark"`. */
  color_mode: string;
  /**
   * MVP-to-v1 migration banner dismissed. Once true, the banner
   * stays hidden across launches even if a legacy notes.db is
   * detected.
   */
  mvp_migration_dismissed: boolean;
}

/** `[secrets]` section. */
export interface UserSecretsBackend {
  backend: string;
  biometric: boolean;
  prompt_timeout_s: number;
}

/** Whole `~/.config/httui/user.toml` document (per-machine). */
export interface UserConfigFile {
  version: "1";
  ui: UserUiPrefs;
  shortcuts: Record<string, string>;
  secrets: UserSecretsBackend;
  mcp: { servers: Record<string, unknown> };
  active_envs: Record<string, string>;
}

export function getWorkspaceConfig(
  vaultPath: string,
): Promise<WorkspaceDefaults> {
  return invoke("get_workspace_config", { vaultPath });
}

export function setWorkspaceConfig(
  vaultPath: string,
  defaults: WorkspaceDefaults,
): Promise<void> {
  return invoke("set_workspace_config", { vaultPath, defaults });
}

export function getUserConfig(): Promise<UserConfigFile> {
  return invoke("get_user_config");
}

export function setUserConfig(file: UserConfigFile): Promise<void> {
  return invoke("set_user_config", { file });
}

/** Outcome of `ensureVaultGitignore`. */
export type GitignoreOutcome = "created" | "augmented" | "already_present";

export function ensureVaultGitignore(
  vaultPath: string,
): Promise<GitignoreOutcome> {
  return invoke("ensure_vault_gitignore", { vaultPath });
}

// --- Vault migration (epic 12) ---

export interface MigrationReport {
  vault_path: string;
  backup_path: string | null;
  connections_migrated: number;
  connections_skipped: number;
  environments_migrated: number;
  environments_skipped: number;
  variables_migrated: number;
  variables_skipped: number;
  dry_run: boolean;
  notes: string[];
}

export function migrateVaultToV1(
  vaultPath: string,
  dryRun: boolean,
): Promise<MigrationReport> {
  return invoke("migrate_vault_to_v1", { vaultPath, dryRun });
}

// --- Vault scaffold + validate (epic 17) ---

export interface ScaffoldReport {
  vault_path: string;
  /** Vault-relative paths the scaffold actually wrote. */
  created: string[];
  already_a_vault: boolean;
}

/** Heuristic: does the folder look like a httui vault? */
export function checkIsVault(vaultPath: string): Promise<boolean> {
  return invoke("check_is_vault", { vaultPath });
}

/** Scaffold default structure under the path. Idempotent. */
export function scaffoldVault(vaultPath: string): Promise<ScaffoldReport> {
  return invoke("scaffold_vault", { vaultPath });
}

// --- Vault operations (V1 vertical 1, cenários 2/3/4) ---
//
// `cloneVault`, `createVault`, `saveSecret` + their IPC types live
// in `./vault-ops.ts` (extracted when commands.ts crossed 600L).
// Re-exported here so existing consumers keep compiling.

export { cloneVault, createVault, saveSecret } from "./vault-ops";
export type { CloneOutcome, CreateOutcome } from "./vault-ops";

// --- Missing secrets scan (epic 18) ---

export interface MissingRef {
  source_file: string;
  label: string;
  keychain_key: string;
  kind: "connection" | "env";
}

/** Scan the vault for `{{keychain:...}}` refs missing from local OS keychain. */
export function listMissingSecrets(vaultPath: string): Promise<MissingRef[]> {
  return invoke("list_missing_secrets", { vaultPath });
}

// --- Filesystem ---

export function listWorkspace(vaultPath: string): Promise<FileEntry[]> {
  return invoke("list_workspace", { vaultPath });
}

export function readNote(vaultPath: string, filePath: string): Promise<string> {
  return invoke("read_note", { vaultPath, filePath });
}

export function forceReloadFile(
  vaultPath: string,
  filePath: string,
): Promise<void> {
  return invoke("force_reload_file", { vaultPath, filePath });
}

export function writeNote(
  vaultPath: string,
  filePath: string,
  content: string,
): Promise<void> {
  return invoke("write_note", { vaultPath, filePath, content });
}

export function createNote(vaultPath: string, filePath: string): Promise<void> {
  return invoke("create_note", { vaultPath, filePath });
}

export function deleteNote(vaultPath: string, filePath: string): Promise<void> {
  return invoke("delete_note", { vaultPath, filePath });
}

export function renameNote(
  vaultPath: string,
  oldPath: string,
  newPath: string,
): Promise<void> {
  return invoke("rename_note", { vaultPath, oldPath, newPath });
}

export function createFolder(
  vaultPath: string,
  folderPath: string,
): Promise<void> {
  return invoke("create_folder", { vaultPath, folderPath });
}

// --- Vault management ---

export async function listVaults(): Promise<string[]> {
  const raw = await getConfig("vaults");
  if (!raw) return [];
  return JSON.parse(raw) as string[];
}

export async function addVault(path: string): Promise<void> {
  const vaults = await listVaults();
  if (!vaults.includes(path)) {
    vaults.push(path);
    await setConfig("vaults", JSON.stringify(vaults));
  }
}

export async function removeVault(path: string): Promise<void> {
  const vaults = await listVaults();
  const filtered = vaults.filter((v) => v !== path);
  await setConfig("vaults", JSON.stringify(filtered));
}

export async function getActiveVault(): Promise<string | null> {
  return getConfig("active_vault");
}

export async function setActiveVault(path: string): Promise<void> {
  await addVault(path);
  await setConfig("active_vault", path);
}

// --- Session restore (single IPC call) ---

export interface SessionTabContent {
  file_path: string;
  vault_path: string;
  content: string | null;
}

export interface SessionState {
  vaults: string[];
  active_vault: string | null;
  vim_enabled: boolean;
  sidebar_open: boolean;
  pane_layout: string | null;
  active_pane_id: string | null;
  active_file: string | null;
  scroll_positions: string | null;
  file_tree: FileEntry[];
  tab_contents: SessionTabContent[];
}

export function restoreSession(): Promise<SessionState> {
  return invoke("restore_session");
}

// --- File watcher ---

export function startWatching(vaultPath: string): Promise<void> {
  return invoke("start_watching", { vaultPath });
}

export function stopWatching(): Promise<void> {
  return invoke("stop_watching");
}

// --- Search ---

export interface SearchResult {
  path: string;
  name: string;
  score: number;
}

export function searchFiles(
  vaultPath: string,
  query: string,
): Promise<SearchResult[]> {
  return invoke("search_files", { vaultPath, query });
}

export interface ContentSearchResult {
  file_path: string;
  snippet: string;
}

export function rebuildSearchIndex(vaultPath: string): Promise<void> {
  return invoke("rebuild_search_index", { vaultPath });
}

export function searchContent(query: string): Promise<ContentSearchResult[]> {
  return invoke("search_content", { query });
}

export function updateSearchEntry(
  filePath: string,
  content: string,
): Promise<void> {
  return invoke("update_search_entry", { filePath, content });
}

// --- Environments ---

export interface Environment {
  id: string;
  name: string;
  is_active: boolean;
  created_at: string;
  /** `[meta].description` (Story 03). Empty/undefined when not set. */
  description?: string | null;
  /** `[meta].temporary` (Story 03). Default false. */
  temporary?: boolean;
  /** `[meta].connections_used` allowlist (Story 03). Empty = all. */
  connections_used?: string[];
}

export interface EnvVariable {
  id: string;
  environment_id: string;
  key: string;
  value: string;
  is_secret: boolean;
  created_at: string;
}

export function listEnvironments(): Promise<Environment[]> {
  return invoke("list_environments");
}

export function createEnvironment(name: string): Promise<Environment> {
  return invoke("create_environment", { name });
}

export function deleteEnvironment(id: string): Promise<void> {
  return invoke("delete_environment", { id });
}

export function duplicateEnvironment(
  sourceId: string,
  newName: string,
): Promise<Environment> {
  return invoke("duplicate_environment", { sourceId, newName });
}

export function renameEnvironment(
  oldId: string,
  newName: string,
): Promise<Environment> {
  return invoke("rename_environment", { oldId, newName });
}

export function setActiveEnvironment(id: string | null): Promise<void> {
  return invoke("set_active_environment", { id });
}

export function listEnvVariables(
  environmentId: string,
): Promise<EnvVariable[]> {
  return invoke("list_env_variables", { environmentId });
}

export function setEnvVariable(
  environmentId: string,
  key: string,
  value: string,
  isSecret?: boolean,
): Promise<EnvVariable> {
  return invoke("set_env_variable", { environmentId, key, value, isSecret });
}

export function deleteEnvVariable(id: string): Promise<void> {
  return invoke("delete_env_variable", { id });
}

/**
 * Resolve every variable of the active environment for execution
 * context. Plain `[vars]` come through verbatim; `[secrets]` are
 * resolved against the OS keychain so HTTP/DB blocks see the real
 * value when expanding `{{KEY}}`.
 *
 * Use ONLY in the request-dispatch path. `listEnvVariables` keeps
 * masking secrets for any display surface.
 */
export function resolveActiveEnvVariables(): Promise<Record<string, string>> {
  return invoke("resolve_active_env_variables");
}

/**
 * Resolve every variable of a specific environment, with secrets
 * unmasked from the OS keychain. Treat the returned map as
 * sensitive — the only legitimate consumers are the request
 * dispatcher and the EnvironmentManager's reveal toggle.
 */
export function resolveEnvVariables(
  environmentId: string,
): Promise<Record<string, string>> {
  return invoke("resolve_env_variables", { environmentId });
}

// --- Block execution ---

export interface BlockResult {
  status: string;
  data: Record<string, unknown>;
  duration_ms: number;
}

export function executeBlock(
  blockType: string,
  params: unknown,
): Promise<BlockResult> {
  return invoke("execute_block", { blockType, params });
}

// --- Block result cache ---

export interface CachedBlockResult {
  status: string;
  response: string;
  total_rows: number | null;
  elapsed_ms: number;
  executed_at: string;
}

export function getBlockResult(
  filePath: string,
  blockHash: string,
): Promise<CachedBlockResult | null> {
  return invoke("get_block_result", { filePath, blockHash });
}

export function saveBlockResult(
  filePath: string,
  blockHash: string,
  status: string,
  response: string,
  elapsedMs: number,
  totalRows?: number | null,
): Promise<void> {
  return invoke("save_block_result", {
    filePath,
    blockHash,
    status,
    response,
    elapsedMs,
    totalRows: totalRows ?? null,
  });
}

// --- Block run history (Story 24.6) ---
//
// Wrappers + types live in `./block-history.ts` (extracted when
// commands.ts crossed the 600-line size gate — Epic 30a Story 07).
// Kept as re-exports here so existing consumer imports
// (`@/lib/tauri/commands`) keep compiling.

export {
  blockHistoryLastRunSummary,
  insertBlockHistory,
  listBlockHistory,
  listBlockHistoryForFile,
  purgeBlockHistory,
} from "./block-history";
export type {
  HistoryEntry,
  InsertHistoryEntry,
  LastRunSummaryRaw,
} from "./block-history";

// --- Per-block settings (Onda 1) ---
//
// Stored in the SQLite `block_settings` table keyed by (file_path, alias).
// All flags are `undefined` when the user never overrode the default — the
// frontend treats absent values as defaults (true for follow_redirects,
// verify_ssl, encode_url, trim_whitespace; false for history_disabled).

export interface HttpBlockSettings {
  followRedirects?: boolean;
  verifySsl?: boolean;
  encodeUrl?: boolean;
  trimWhitespace?: boolean;
  historyDisabled?: boolean;
}

/** Reads settings; missing row → all-undefined object (use defaults). */
export function getBlockSettings(
  filePath: string,
  blockAlias: string,
): Promise<HttpBlockSettings> {
  return invoke("get_block_settings", { filePath, blockAlias });
}

/** Upserts settings. Pass `undefined` for any flag to revert it to default. */
export function upsertBlockSettings(
  filePath: string,
  blockAlias: string,
  settings: HttpBlockSettings,
): Promise<void> {
  return invoke("upsert_block_settings", { filePath, blockAlias, settings });
}

/** Removes the row entirely. Used as cascade when a block is deleted. */
export function purgeBlockSettings(
  filePath: string,
  blockAlias: string,
): Promise<number> {
  return invoke("purge_block_settings", { filePath, blockAlias });
}

// --- Pinned response examples (Onda 3) ---

export interface BlockExample {
  id: number;
  file_path: string;
  block_alias: string;
  name: string;
  response_json: string;
  saved_at: string;
}

/** Save (or replace by `name`) a response snapshot as an example. */
export function saveBlockExample(
  filePath: string,
  blockAlias: string,
  name: string,
  responseJson: string,
): Promise<number> {
  return invoke("save_block_example", {
    filePath,
    blockAlias,
    name,
    responseJson,
  });
}

/** List examples for a (file, alias), most recent first. */
export function listBlockExamples(
  filePath: string,
  blockAlias: string,
): Promise<BlockExample[]> {
  return invoke("list_block_examples", { filePath, blockAlias });
}

/** Delete a single example by id. */
export function deleteBlockExample(id: number): Promise<number> {
  return invoke("delete_block_example", { id });
}

/** Cascade-delete all examples for a (file, alias). Used when removing a block. */
export function purgeBlockExamples(
  filePath: string,
  blockAlias: string,
): Promise<number> {
  return invoke("purge_block_examples", { filePath, blockAlias });
}
