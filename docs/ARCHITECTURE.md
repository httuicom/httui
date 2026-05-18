# httui-notes — Architecture

> Reflects the v1 file-backed, git-native architecture. The
> foundation work for v1 is in place across the storage and secrets
> layers; the React panels that consume the new file-backed stores
> are still being cut over from the legacy SQLite path.

## TL;DR

httui-notes is a desktop markdown editor built on Tauri (Rust
backend + React frontend). It stores **runbooks + config in plain
files**, syncs via **git**, and keeps **secrets in the OS
keychain**. SQLite still ships, but only as cache and ephemeral
state.

```
Desktop app ─┐
TUI binary  ─┼─ all read the same vault on disk
MCP server  ─┘   (git repo with .md + .toml)
```

## Crates and components

| Path | Role |
|---|---|
| `httui-core/` | Pure Rust shared library: parsers, executors, vault config, secrets, git CLI wrapper. No GUI deps. |
| `httui-desktop/src-tauri/` | Tauri v2 backend. Wires Tauri commands, owns the running watcher + chat sidecar. |
| `httui-desktop/src/` | React + TypeScript frontend. CodeMirror 6 editor, Chakra UI v3, Zustand stores. |
| `httui-tui/` | Terminal binary (read-only viewer + executor for v1; full editor TUI is a future-scope item). |
| `httui-mcp/` | MCP server binary. 14 tools (list/read/create/update notes, search, connections, environments). |
| `httui-sidecar/` | Node.js process spawned by the desktop app for the chat feature (Claude Agent SDK). |
| `httui-web/` | Marketing landing page (separate Vite app). |

## Vault layout

The vault is a regular git repo. httui adds a `.httui/` directory
plus a few well-known config files at the root:

```
my-vault/
├── runbooks/                    # .md files with executable blocks
├── connections.toml             # shared connection definitions
├── connections.local.toml       # personal override (gitignored)
├── envs/
│   ├── local.toml
│   ├── staging.toml
│   ├── staging.local.toml       # personal override (gitignored)
│   └── prod.toml
├── .httui/
│   ├── workspace.toml           # workspace defaults (committed)
│   └── workspace.local.toml     # personal override (gitignored)
├── .gitignore                   # auto-includes *.local.toml block
└── notes.db                     # SQLite cache (gitignored)
```

The committed files are reviewable as PR diffs. The `.local.toml`
siblings deep-merge over their base on read; writes always target
the base file (ADR 0004).

Per-machine prefs (theme, font, density, keybindings, secrets
backend, MCP toggles) live in `~/.config/httui/user.toml` (XDG
respected on Linux; OS-native config dir elsewhere).

## What's a file vs what's SQLite

| Data | Lives in | Synced via git |
|---|---|---|
| Runbooks (`.md`) | repo | yes |
| Connection definitions | `connections.toml` | yes |
| Connection passwords | OS keychain | no (per machine) |
| Env vars (non-secret) | `envs/<name>.toml` `[vars]` | yes |
| Env vars (secret) | OS keychain (TOML carries `{{keychain:...}}` ref) | no |
| Personal overrides | `*.local.toml` | no (gitignored) |
| Workspace defaults | `.httui/workspace.toml` | yes |
| Per-machine prefs | `~/.config/httui/user.toml` | no |
| Run history | SQLite `block_run_history` | no |
| Block result cache | SQLite `block_result` | no |
| Schema introspection | SQLite `schema_cache` | no |
| Chat sessions | SQLite `sessions` / `messages` | no |
| Active vault / pane layout / scroll positions | SQLite `app_config` | no |

The seven UI prefs keys (theme, auto_save_ms, editor_font_size,
default_fetch_size, history_retention, vim_enabled, sidebar_open)
were kept in SQLite during the MVP; the v1 migration moves them to
`user.toml [ui]`. Session-state keys (`vaults`, `active_vault`,
`pane_layout`, `active_pane_id`, `active_file`, `scroll_positions`)
**stay in SQLite** because they're per-keystroke writes.

## Code shape — `httui-core/src/vault_config/`

The file-backed config layer ships in this module:

| File | Role |
|---|---|
| `connections_store.rs` | CRUD on `connections.toml` via the keychain. Mtime-cached. |
| `environments_store.rs` | CRUD on `envs/*.toml` and active-env tracking in `user.toml`. |
| `workspace_store.rs` | CRUD on `.httui/workspace.toml`. |
| `user_store.rs` | CRUD on `~/.config/httui/user.toml` with XDG resolution. |
| `merge.rs` | Deep-merge for `*.local.toml` overrides (ADR 0004). |
| `gitignore.rs` | Auto-augments the vault `.gitignore` with the canonical `*.local.toml` patterns. |
| `migration.rs` | One-shot migration from MVP SQLite tables to the file layout. Idempotent + dry-run + backup. |
| `missing_secrets.rs` | First-run scanner for `{{keychain:...}}` refs that aren't yet populated. |
| `scaffold.rs` | Default vault skeleton + `is_vault()` heuristic. |
| `watch_paths.rs` | Pure path classifier consumed by the watcher (Connections / Env / Workspace). |
| `validate.rs` | Anti-cleartext-secret check + structural validation. |
| `atomic.rs` | Atomic-write helper (temp file + fsync + rename, ADR 0003). |

All stores cache by `(base_mtime, local_mtime)` so external edits
to either side invalidate correctly. Mutating paths read **base
only** to avoid promoting overrides into the committed file
(audit-003).

## Secret resolution

`{{backend:address}}` references in TOML resolve at read time
through a `SecretBackend` trait (`httui-core/src/secrets/`). The
default impl is `Keychain` (delegates to `keyring` crate / OS
keychain). Future backends — Touch ID, Windows Hello, 1Password CLI,
pass — slot in behind the same trait without callsite changes
(Epics 14-16).

The parser (`secrets/parser.rs`) recognises four backend prefixes:
`keychain`, `1password`, `pass`, `env`. Anything else is a parse
error.

The validator rejects raw secret values written to `[secrets]`
sections — a hard error, not a warning. The escape hatch is
`# httui:allow-cleartext` per ADR 0002.

## File watcher

Single OS-level watcher (notify crate, recursive) at the vault
root. The dispatcher routes events:

- `*.md` → existing `file-reloaded` flow (read content, emit to
  frontend, ConflictBanner if dirty)
- watched config TOML (per `watch_paths::classify`) → emit
  `config-changed` event with `{ category, path, env? }`. Stores
  invalidate caches by mtime; the frontend cutover (Epic 19) will
  add a `Store::invalidate_cache()` call on event receipt.

Debounce: 500 ms for `.md`, 250 ms for TOML (ADR 0003).

## Process model

```
┌──────────────────────────┐    ┌──────────────────────────┐
│  Tauri main (Rust)       │    │  Node.js sidecar         │
│                          │ ◀──┤  (Claude Agent SDK)      │
│  - executor registry     │    │                          │
│  - file watcher          │    │  spawned on first chat   │
│  - keychain bridge       │    │  message; NDJSON over    │
│  - chat permission broker│    │  stdin/stdout            │
└──────────────────────────┘    └──────────────────────────┘
            ▲
            │ Tauri IPC (invoke / Channel)
            ▼
┌──────────────────────────┐
│  React + Vite (webview)  │
│                          │
│  - CodeMirror 6 editor   │
│  - Chakra UI panels      │
│  - Zustand stores        │
└──────────────────────────┘
```

The sidecar is health-checked via ping/pong and respawns on
failure with exponential backoff. The chat permission broker
(`httui-desktop/src-tauri/src/chat/permissions.rs`) intercepts
tool calls before prompting the user.

## ADRs

Architecture decisions live under [`docs/adr/`](./adr/):

- [0001 — TOML schemas](./adr/0001-toml-schemas.md)
- [0002 — Secret references](./adr/0002-secret-references.md)
- [0003 — File watcher](./adr/0003-file-watcher.md)
- [0004 — Local overrides](./adr/0004-local-overrides.md)

Future decisions go through the same template (Status / Context /
Decision / Consequences / References).

## What's NOT here (out of scope for v1)

The deliberately excluded surface:

- No web app — desktop + TUI only.
- No CLI runner — `httui run runbook.md --env=staging` is a v2 idea.
- No Docker self-host — vault is a git repo; that's the sync server.
- No formal block-execution lifecycle redesign — MVP "run / cancel"
  is sufficient for v1.

## Where to start as a contributor

1. `httui-core/src/blocks/parser.rs` — markdown → block AST.
2. `httui-core/src/executor/{http,db}/` — block execution paths.
3. `httui-core/src/vault_config/` — the storage layer described above.
4. `httui-desktop/src/components/blocks/` — React panels for HTTP / DB.
5. `httui-desktop/src/stores/` — Zustand state.

`CLAUDE.md` at the repo root has the running architecture notes
the AI agents use; it's intentionally more granular than this file
and tracks the actual file paths + line counts for hot modules.
