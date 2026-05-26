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
  return invoke<string | null>("get_config", { key });
}

export function setConfig(key: string, value: string): Promise<void> {
  return invoke<void>("set_config", { key, value });
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
 * than just a mode string; the migration writes a bare
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
  /** Git side-panel (VS-Code-style SCM) open/closed. V10.1. */
  git_side_panel_open?: boolean;
  /** Commit-message template for the git side panel. V10.1. */
  git_commit_template?: string;
  /** Color mode: `"system"` | `"light"` | `"dark"`. */
  color_mode: string;
  /**
   * MVP-to-v1 migration banner dismissed. Once true, the banner
   * stays hidden across launches even if a legacy notes.db is
   * detected.
   */
  mvp_migration_dismissed: boolean;
  /** Opt-in to pre-release auto-updates (`-rc`/`-beta`/`-alpha`). V12. */
  auto_update_include_prereleases?: boolean;
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
  return invoke<WorkspaceDefaults>("get_workspace_config", { vaultPath });
}

export function setWorkspaceConfig(
  vaultPath: string,
  defaults: WorkspaceDefaults,
): Promise<void> {
  return invoke<void>("set_workspace_config", { vaultPath, defaults });
}

export function getUserConfig(): Promise<UserConfigFile> {
  return invoke<UserConfigFile>("get_user_config");
}

export function setUserConfig(file: UserConfigFile): Promise<void> {
  return invoke<void>("set_user_config", { file });
}

/** Outcome of `ensureVaultGitignore`. */
export type GitignoreOutcome = "created" | "augmented" | "already_present";

export function ensureVaultGitignore(
  vaultPath: string,
): Promise<GitignoreOutcome> {
  return invoke<GitignoreOutcome>("ensure_vault_gitignore", { vaultPath });
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
  return invoke<MigrationReport>("migrate_vault_to_v1", { vaultPath, dryRun });
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
  return invoke<boolean>("check_is_vault", { vaultPath });
}

/** Scaffold default structure under the path. Idempotent. */
export function scaffoldVault(vaultPath: string): Promise<ScaffoldReport> {
  return invoke<ScaffoldReport>("scaffold_vault", { vaultPath });
}

// --- Vault operations -----------------------------------
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
  return invoke<MissingRef[]>("list_missing_secrets", { vaultPath });
}

// --- Filesystem ---

export function listWorkspace(vaultPath: string): Promise<FileEntry[]> {
  return invoke<FileEntry[]>("list_workspace", { vaultPath });
}

export function readNote(vaultPath: string, filePath: string): Promise<string> {
  return invoke<string>("read_note", { vaultPath, filePath });
}

export function forceReloadFile(
  vaultPath: string,
  filePath: string,
): Promise<void> {
  return invoke<void>("force_reload_file", { vaultPath, filePath });
}

export function writeNote(
  vaultPath: string,
  filePath: string,
  content: string,
): Promise<void> {
  return invoke<void>("write_note", { vaultPath, filePath, content });
}

export function createNote(vaultPath: string, filePath: string): Promise<void> {
  return invoke<void>("create_note", { vaultPath, filePath });
}

export function deleteNote(vaultPath: string, filePath: string): Promise<void> {
  return invoke<void>("delete_note", { vaultPath, filePath });
}

export function renameNote(
  vaultPath: string,
  oldPath: string,
  newPath: string,
): Promise<void> {
  return invoke<void>("rename_note", { vaultPath, oldPath, newPath });
}

export function createFolder(
  vaultPath: string,
  folderPath: string,
): Promise<void> {
  return invoke<void>("create_folder", { vaultPath, folderPath });
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
  return invoke<SessionState>("restore_session");
}

// --- File watcher ---

export function startWatching(vaultPath: string): Promise<void> {
  return invoke<void>("start_watching", { vaultPath });
}

export function stopWatching(): Promise<void> {
  return invoke<void>("stop_watching");
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
  return invoke<SearchResult[]>("search_files", { vaultPath, query });
}

export interface ContentSearchResult {
  file_path: string;
  snippet: string;
}

export function rebuildSearchIndex(vaultPath: string): Promise<void> {
  return invoke<void>("rebuild_search_index", { vaultPath });
}

export function searchContent(query: string): Promise<ContentSearchResult[]> {
  return invoke<ContentSearchResult[]>("search_content", { query });
}

export function updateSearchEntry(
  filePath: string,
  content: string,
): Promise<void> {
  return invoke<void>("update_search_entry", { filePath, content });
}

// --- Environments + executable blocks ---
//
// Wrappers + types live in `./env-block-commands.ts` (extracted when
// commands.ts crossed the 600-line size gate — the `invoke<T>` typing
// pass reflowed several call sites past the limit). Re-exported here so
// existing consumer imports (`@/lib/tauri/commands`) keep compiling.

export * from "./env-block-commands";
