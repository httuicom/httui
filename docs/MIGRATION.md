# Migration guide — legacy SQLite layout → file-backed layout

This guide is for users who installed an early **MVP build** of httui
(when configuration lived entirely inside `notes.db`) and want to move
to the current **file-backed layout**, where connections, environments
and per-machine prefs live in plain TOML files inside the vault.
("MVP" and "v1" below are the internal names of those two storage
layouts, not public version numbers.)

> **Status note (April 2026).** The v1 storage layer is in place and
> the migration tooling described below is shippable. The React panels
> still read from the legacy SQLite tables until the frontend cutover
> lands. Until that cutover is published, the safe upgrade path is
> **inspect-only**: migrate, review the generated TOML, but expect
> the running app to keep using SQLite as the source of truth.

---

## What changes

| Before (MVP) | After (v1) |
|---|---|
| Connections in `notes.db` `connections` table | `connections.toml` at the vault root |
| Environments + variables in `notes.db` `environments` / `env_variables` | `envs/<name>.toml` per environment |
| Connection passwords + secret variables stored in OS keychain (already), referenced by sentinel `__KEYCHAIN__` in SQLite | Same OS keychain, referenced by `{{keychain:…}}` markers in TOML |
| UI prefs (`theme`, `auto_save_ms`, `editor_font_size`, `default_fetch_size`, `history_retention`, `vim_enabled`, `sidebar_open`) in `app_config` table | `~/.config/httui/user.toml` `[ui]` section (XDG on Linux; OS-native config dir elsewhere) |
| Session state (`vaults`, `active_vault`, `pane_layout`, …) in `app_config` | **Stays in `app_config`** — session state is per-machine ephemeral; it never leaves SQLite (audit-001) |
| Run history, schema cache, block result cache | **Stays in SQLite** — these are caches, intentionally not committed |

The full target layout is documented in
[`docs/ARCHITECTURE.md`](./ARCHITECTURE.md#vault-layout).

## Before you start

1. **Close the app.** Don't run the migration while httui is open —
   the migration needs an exclusive read of `notes.db`.
2. **Commit the vault.** The migration writes new files into the vault
   root; a clean working tree makes the diff trivial to review.
3. **Update httui.** Install the current build over your old install. Your
   data is untouched until you trigger the migration.

## Run the migration

The migration is invoked from inside the desktop app via the Tauri
command `migrate_vault_to_v1`. It is **idempotent**: rerunning is safe.

| Argument | Meaning |
|---|---|
| `vault_path` | Absolute path of the vault to migrate. Required. |
| `dry_run` | `true` to walk the SQLite tables and report what would be written, without touching any file. Set this first to preview. |

A successful run returns a `MigrationReport` with these counters:

| Field | Meaning |
|---|---|
| `vault_path` | Vault that was migrated |
| `backup_path` | Where `notes.db` was copied before any write (typically `<vault>/notes.db.pre-v1-backup`). `null` on dry-run. |
| `connections_migrated` / `connections_skipped` | New `connections.toml` rows vs. duplicates already present |
| `environments_migrated` / `environments_skipped` | New env files vs. duplicates |
| `variables_migrated` / `variables_skipped` | Env-var rows added vs. duplicates |
| `prefs_migrated` | Number of UI-pref keys copied into `user.toml` `[ui]` |
| `dry_run` | Mirrors the input flag |
| `notes` | Free-form messages — backup status, dual-storage warning, etc. |

### Recommended sequence

1. **Dry-run first** with `dry_run = true`. Verify the counters match
   what you expect from the MVP app (connection count, environment
   count, variable count).
2. **Run for real** with `dry_run = false`. The migration:
   - Copies `notes.db` → `notes.db.pre-v1-backup` (only if the
     database exists; no-op on dry-run).
   - Walks the `connections`, `environments`, `env_variables` and the
     seven UI-pref keys in `app_config`, then writes the corresponding
     TOML files.
   - Returns the report. Re-running on an already-populated vault
     **does not duplicate** entries — duplicate inserts are folded into
     the `*_skipped` counters.
3. **Inspect the diff.** `git status` should show new files at the
   vault root: `connections.toml`, `envs/*.toml`. Check that the values
   match your MVP setup. Secrets appear as `{{keychain:…}}` markers,
   never as plaintext.
4. **Commit.** The migration deliberately does **not** auto-stage. Add
   only after you have verified the diff yourself — this avoids
   accidentally committing a half-encrypted secret if the keychain
   backend silently failed during a previous run.

### Backups

The first non-dry-run produces `notes.db.pre-v1-backup` next to
`notes.db`. Subsequent runs overwrite that backup (the SQLite content
hasn't changed in a meaningful way — the migration is one-way only).
If you want a frozen point-in-time copy, take it manually before the
first run.

## After migrating

Until the frontend cutover ships, the running app keeps reading
from the legacy SQLite tables:

- The TOML files are valid and the migration is the source of truth
  for v1.x onwards, but the app's UI hasn't been switched over yet.
- Editing a connection inside the app today still goes to SQLite, not
  to `connections.toml`. The two states will diverge if you keep
  editing — re-run the migration whenever you want to refresh the
  TOML side.
- After the cutover lands, the legacy SQLite tables for connections,
  environments and `env_variables` will be dropped and the TOML files
  become the only source of truth. The keychain integration is
  unchanged across the cutover.

The seven UI-pref keys (`theme`, `auto_save_ms`, `editor_font_size`,
`default_fetch_size`, `history_retention`, `vim_enabled`,
`sidebar_open`) are already safe to drop from `app_config` — the
schema bump did this for new installs; on upgraded vaults the keys
linger harmlessly.

## Secrets

Connection passwords and secret environment variables continue to live
in the OS keychain. The migration does **not** re-encrypt or re-prompt:

- Passwords created in the MVP are still in the keychain under the
  same service/account used by the MVP build. The new TOML files
  reference them with `{{keychain:…}}` markers; lookups go through
  the same `keyring` crate.
- If your machine ever lost keychain access (e.g. a corrupted login
  keychain on macOS), the MVP fell back to plaintext. After
  migration, you'll see those values appear inline in the TOML —
  re-enter them through the app to push them back into the keychain.

First-run secret setup on a freshly cloned vault is handled by the
`first_run_missing_secrets` Tauri command. The flow:

1. Open vault → app scans `connections.toml` + `envs/*.toml` for
   `{{keychain:…}}` markers without a corresponding keychain entry on
   this machine.
2. Reports the list to the UI; user enters the values once.

## Troubleshooting

**Migration fails with "backup notes.db: Permission denied".** The
process can't write next to `notes.db`. Check that the vault folder
is writable; on macOS, also check that the app has Full Disk Access
or that the vault sits outside `~/Library`/`~/Documents/...` quarantine
zones.

**Counters say `0 migrated` even though MVP had data.** The migration
read an empty `notes.db` (probably the wrong vault path was passed).
Re-check `vault_path`; the right one is the directory that contains
`notes.db` plus your `runbooks/` folder.

**Re-running raises duplicate-name errors in the report.** Expected
behaviour — duplicates fold into the `*_skipped` counters; the report
keeps growing as you add new entries to SQLite and re-run. There is
no destructive collision case.

**Some env values show as `{{keychain:…}}` in `envs/*.toml` but the
app prompts for them again.** Either the keychain was reset between
the MVP install and the v1 install, or the v1 build runs as a
different user/profile. Re-enter the secret in the app once; the new
keychain entry sticks.

**I want to roll back to MVP storage.** Restore `notes.db` from
`notes.db.pre-v1-backup` and delete the new TOML files. The MVP
build reads from `notes.db` exclusively; nothing else needs to be
undone.

## Sharing — no share modal

Earlier mockups showed a "Share" modal with snapshot links, live
links, expirations and generated passwords. httui ships **none of
that**. Sharing a vault is sharing the git repo — clone it,
permission it through your hosting provider (GitHub, GitLab, …), and
each collaborator's secrets stay on their machine via the keychain
flow already covered above. The git panel exposes "Copy repo URL",
"Copy permalink at current commit", and "Open pull request" as
one-click actions over the configured `origin` remote.

See [`docs/concepts.md` → Sharing a vault](./concepts.md#sharing-a-vault)
for the full picture.

## Reference

- ADR 0001 — TOML schemas: `docs/adr/0001-toml-schemas.md`
- ADR 0002 — secret references: `docs/adr/0002-secret-references.md`
- ADR 0004 — local override mechanism: `docs/adr/0004-local-overrides.md`
