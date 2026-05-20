// coverage:exclude file — Pure `invoke()` wrappers + IPC type
// declarations for environments + executable blocks. Extracted from
// `commands.ts` when it crossed the 600-line size gate (the `invoke<T>`
// typing pass reflowed several call sites past the limit). Re-exported
// from `commands.ts` so existing `@/lib/tauri/commands` imports keep
// compiling. Testing these is testing the mock harness; the real
// behavior lives in the backend. Documented in tech-debt.md and
// audit-002.

import { invoke } from "@tauri-apps/api/core";

// --- Environments ---

export interface Environment {
  id: string;
  name: string;
  is_active: boolean;
  created_at: string;
  /** `[meta].description`. Empty/undefined when not set. */
  description?: string | null;
  /** `[meta].temporary`. Default false. */
  temporary?: boolean;
  /** `[meta].connections_used` allowlist. Empty = all. */
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
  return invoke<Environment[]>("list_environments");
}

export function createEnvironment(name: string): Promise<Environment> {
  return invoke<Environment>("create_environment", { name });
}

export function deleteEnvironment(id: string): Promise<void> {
  return invoke<void>("delete_environment", { id });
}

export function duplicateEnvironment(
  sourceId: string,
  newName: string,
): Promise<Environment> {
  return invoke<Environment>("duplicate_environment", { sourceId, newName });
}

export function renameEnvironment(
  oldId: string,
  newName: string,
): Promise<Environment> {
  return invoke<Environment>("rename_environment", { oldId, newName });
}

export function setActiveEnvironment(id: string | null): Promise<void> {
  return invoke<void>("set_active_environment", { id });
}

export function listEnvVariables(
  environmentId: string,
): Promise<EnvVariable[]> {
  return invoke<EnvVariable[]>("list_env_variables", { environmentId });
}

export function setEnvVariable(
  environmentId: string,
  key: string,
  value: string,
  isSecret?: boolean,
): Promise<EnvVariable> {
  return invoke<EnvVariable>("set_env_variable", {
    environmentId,
    key,
    value,
    isSecret,
  });
}

export function deleteEnvVariable(id: string): Promise<void> {
  return invoke<void>("delete_env_variable", { id });
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
  return invoke<Record<string, string>>("resolve_active_env_variables");
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
  return invoke<Record<string, string>>("resolve_env_variables", {
    environmentId,
  });
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
  return invoke<BlockResult>("execute_block", { blockType, params });
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
  return invoke<CachedBlockResult | null>("get_block_result", {
    filePath,
    blockHash,
  });
}

export function saveBlockResult(
  filePath: string,
  blockHash: string,
  status: string,
  response: string,
  elapsedMs: number,
  totalRows?: number | null,
): Promise<void> {
  return invoke<void>("save_block_result", {
    filePath,
    blockHash,
    status,
    response,
    elapsedMs,
    totalRows: totalRows ?? null,
  });
}

// --- Block run history ----------------
//
// Wrappers + types live in `./block-history.ts` (extracted when
// commands.ts crossed the 600-line size gate). Kept as re-exports here
// so existing consumer imports (`@/lib/tauri/commands`) keep compiling.

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
  return invoke<HttpBlockSettings>("get_block_settings", {
    filePath,
    blockAlias,
  });
}

/** Upserts settings. Pass `undefined` for any flag to revert it to default. */
export function upsertBlockSettings(
  filePath: string,
  blockAlias: string,
  settings: HttpBlockSettings,
): Promise<void> {
  return invoke<void>("upsert_block_settings", {
    filePath,
    blockAlias,
    settings,
  });
}

/** Removes the row entirely. Used as cascade when a block is deleted. */
export function purgeBlockSettings(
  filePath: string,
  blockAlias: string,
): Promise<number> {
  return invoke<number>("purge_block_settings", { filePath, blockAlias });
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
  return invoke<number>("save_block_example", {
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
  return invoke<BlockExample[]>("list_block_examples", {
    filePath,
    blockAlias,
  });
}

/** Delete a single example by id. */
export function deleteBlockExample(id: number): Promise<number> {
  return invoke<number>("delete_block_example", { id });
}

/** Cascade-delete all examples for a (file, alias). Used when removing a block. */
export function purgeBlockExamples(
  filePath: string,
  blockAlias: string,
): Promise<number> {
  return invoke<number>("purge_block_examples", { filePath, blockAlias });
}
