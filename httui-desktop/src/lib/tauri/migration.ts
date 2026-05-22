// Migration-related Tauri wrappers — MVP-to-v1 migration banner detection.
// Coverage via consumer hook (`useMigrationDetection`) that mocks the Tauri command names.

import { invoke } from "@tauri-apps/api/core";

/** Mirror of `httui_core::vault_config::migration::MigrationCandidate`.
 * The backend reports both flags so the frontend can also display
 * "v1 already initialised" hints; `should_prompt()` lives on the
 * Rust side and is mirrored here as `shouldPromptMigration`. */
export interface MigrationCandidate {
  has_legacy_db: boolean;
  has_v1_layout: boolean;
}

/** True iff a legacy `notes.db` is present and the v1 `.httui/`
 * layout has not been initialised. Frontend gates the banner on
 * this AND the `mvpMigrationDismissed` user pref. Mirrors the
 * `MigrationCandidate::should_prompt` Rust helper. */
export function shouldPromptMigration(c: MigrationCandidate): boolean {
  return c.has_legacy_db && !c.has_v1_layout;
}

/** Probe `vaultPath` to decide whether to surface the MVP→v1
 * migration banner. Pure `invoke()` shell over
 * `detect_vault_migration` (`vault_config_commands.rs`). */
export function detectVaultMigration(
  vaultPath: string,
): Promise<MigrationCandidate> {
  return invoke("detect_vault_migration", { vaultPath });
}

/** Mirror of `httui_core::vault_config::migration::MigrationReport`.
 * Counts reflect what was actually written (or, on a dry run, what
 * would be). Backed by `migrate_vault_to_v1`. */
export interface MigrationReport {
  vault_path: string;
  backup_path: string | null;
  connections_migrated: number;
  connections_skipped: number;
  environments_migrated: number;
  environments_skipped: number;
  variables_migrated: number;
  variables_skipped: number;
  prefs_migrated: number;
  dry_run: boolean;
  notes: string[];
}

/** Migrate the MVP-era SQLite vault to the v1 file layout. Set
 * `dryRun=true` to preview without writing. Pure `invoke()` shell
 * over `migrate_vault_to_v1` (`vault_config_commands.rs`). */
export function migrateVaultToV1(
  vaultPath: string,
  dryRun: boolean,
): Promise<MigrationReport> {
  return invoke("migrate_vault_to_v1", { vaultPath, dryRun });
}
