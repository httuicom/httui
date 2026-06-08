# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog 1.1](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Post-0.4.3 work lands here.

## [0.4.3] - 2026-06-08

### Fixed

- **Auto-update gap**: the desktop now self-links the `httui` terminal launcher into `~/.local/bin/httui` on launch. Users who upgraded from 0.4.1/0.4.2 via Tauri's in-app updater (which can't touch `/usr/local/bin`) get `httui` on `PATH` the next time they open the app, without re-running the installer. Idempotent and never overrides a working `httui` already on `PATH`.

## [0.4.2] - 2026-06-08

### Added

- **Distribution**: TUI ships in the same bundle as the desktop. Every release (`.dmg`, `.deb`, `.rpm`, `.msi`) now contains `httui-desktop`, `httui-tui`, and a unified `httui` launcher. Running `httui` in a terminal opens the TUI; `httui desktop` opens the desktop app; double-clicking the `.app`/`.exe` keeps opening the desktop directly. The Homebrew cask symlinks `httui` into `/usr/local/bin` automatically; `.deb`/`.rpm` install all three to `/usr/bin/`. Local install adds `make install-tui` for the symlink. See `docs/RELEASE.md §7a`.
- **TUI**: full git surface inside the terminal — `Ctrl+G` opens a right-side git panel mirroring the desktop's SCM column. Status (UNSTAGED/STAGED file lists), commit form with `{{notes}}/{{count}}/{{date}}` template (shared with desktop via `user.toml [ui].git_commit_template`), 1-click Sync (`Ctrl+Enter`, stage→commit→pull `--ff-only`→push) with a confirm modal when the branch has no upstream, branch picker (`Ctrl+B`), full-screen log + diff viewer (`Ctrl+L`), 3-way conflict resolver (`Ctrl+R`, `1`/`2`/`3` pick base/ours/theirs), share URL (`Ctrl+Y` copies HTTPS), amend toggle (`Ctrl+A`), and conflict-marker highlighting in the editor.
- **TUI**: status bar shows the current branch + ahead/behind chip permanently when the vault is a git repo.
- **TUI**: in-app Settings page (`Alt+,`) — rebind keymaps, pick a theme (3 presets + per-color overrides in `config.toml`), toggle vim ↔ standard mode. ([#61](https://github.com/httuicom/httui/pull/61))

## [0.4.1] - 2026-05-21

Maintenance release: installer, offline fonts, bug fixes.

### Added

- **One-line installer** — `curl -fsSL https://httui.com/install.sh | sh` installs the latest macOS or Linux build.

### Changed

- **Self-hosted fonts** — fonts are bundled instead of fetched from the Google Fonts CDN; the app now works fully offline.

### Fixed

- **Editor** — leaked focus listeners when switching files or toggling vim.
- **HTTP block** — visible flash from the body editor reconfiguring on every blur in form mode.
- **Connections** — save failures in the new-connection modal now show an error instead of failing silently.

### Added

- **TUI**: standard (non-modal) edit mode is now the default profile — arrow keys move the cursor, `Ctrl+Z/Y` undo/redo, `Ctrl+C/X/V` copy/cut/paste, `Home/End/PageUp/PageDown` navigate, `Shift+arrow`/`Shift+Home`/`Shift+End` extend selection, `Ctrl+S` saves. Users can edit without any knowledge of vim. `Ctrl+Shift+X` runs EXPLAIN on the focused DB block (vim still uses bare `Ctrl+X`).
- **TUI**: vim mode is now opt-in via `editor.mode = "vim"` in the config — the modal vim engine is preserved unchanged for users who prefer it.
- **TUI**: `Ctrl+Shift+M` hot-toggles between standard and vim at runtime, in any mode (Normal/Insert/Visual/Cmdline/Search), without restarting. Transient input state (vim pending operators, standard selection anchor) is reset on toggle.
- **TUI**: auto-save (1s debounce after the last edit) in standard mode, plus an unconditional flush before quit so nothing is lost on `:q` or `Ctrl+C` shutdown.
- **TUI**: inspectable keymap data layer (`input::map`) — every chord-to-Action binding for the standard profile lives in a single table; the vim profile's flat chords are listed documentary-style. Foundation for the Settings keymap UI in V9.
- **TUI**: `/` in standard-mode prose opens the block-template picker (HTTP GET, HTTP POST JSON, SQLite query). Vim keeps the `gN` chord; both routes land on the same picker. Pressing Enter on a template splices the fence at the cursor and the parser promotes it to a block.
- **TUI**: Variables + Environments management surface (`Alt+i` / `gV`) — master-detail page with the envs sidebar and a per-env vars table. Create/rename/delete envs and vars in place, toggle `is_secret` (values masked as `••••` in the list, raw in edit), `c` clones an env with a per-variable checkbox to pick which keys to copy. Reads/writes `<vault>/envs/*.toml` via `httui_core::EnvironmentsStore`; secret values are stored in the OS keychain.
- **TUI**: numeric shortcuts `1`-`9` activate the env at that position from either the `gE` picker or anywhere in the Variables/Envs page (regardless of which pane is focused). After activating, focus lands on the new env's vars so the user can edit values right away.
- **TUI**: per-variable "Used in N" panel — the Variables page shows where the selected var (`{{KEY}}` or `{{KEY.path}}`) is referenced across every `.md` in the vault, with file:line and a snippet. Powered by `httui_core::var_uses`.
- **TUI**: empty-state on first run — when no vault is registered the binary opens a ratatui screen with three cards (Open / Clone / Create) instead of a stdin prompt. Open browses a directory tree by keyboard, Clone runs `git clone` into a chosen parent, Create scaffolds a fresh vault with `git init`. The chosen vault is persisted and the workbench opens normally.
- **TUI**: pending-secrets first-run modal — when switching to a vault whose `{{keychain:X}}` references have no entry in the OS keychain, a modal lists each missing key with an inline input. Enter saves to the keychain, `s` skips (leaving the badge on the status bar), Esc dismisses. A "⚠ N pending" badge surfaces remaining items and can reopen the modal.
- **Build**: `commit-msg` git hook enforces the project commit style (subject only, ≤72 chars, no internal planning vocabulary, no AI-assistant attribution). Install with `make setup-hooks`.
- **TUI**: HTTP result panel renames the four legacy DB-shaped tabs to `Body / Headers / Cookies / Timing` and adds a fifth `Raw` tab that paints the response as the wire HTTP-message (status line + headers + blank + body). Cycle with `gt`/`gT` or `Tab`/`Shift+Tab`.
- **TUI**: HTTP body viewer now picks a highlighter from the `Content-Type` response header — JSON (existing), XML, HTML (basic), plain otherwise.
- **TUI**: HTTP requests stream through `execute_streamed`. The status bar shows live `↓ X kB · Y s` while the body is being received, so multi-MB downloads no longer look frozen.
- **TUI**: Tab/Shift+Tab cycle the focused block's result-panel tab (Body→Headers→Cookies→Timing→Raw on HTTP, four tabs on DB). Per-block state — cycling one block no longer drags every other block's tab along.
- **TUI**: VarForm / EnvForm fields support inline cursor navigation — Left/Right/Home/End move the caret, Delete forward-deletes, Backspace continues to back-delete.
- **TUI**: per-connection session host/port override on the Connections page — press `o` to open the override form (prefilled with the connection's stored host/port), `O` to clear. Active overrides surface a `TEMP` amber badge in the sidebar and an amber "Session override (TEMP)" section in the detail pane. In-memory only — never persisted, disappears on restart. Cache is bypassed while an override is active (same SQL against staging vs prod won't share a cache slot).
- **TUI**: DB blocks that error now keep the error message inside the result panel (instead of only on the status bar that scrolls away on the next keystroke). Pressing Enter anywhere in the SQL body of an errored block opens the detail modal so the message can be navigated and copied.
- **TUI**: MySQL connections now negotiate `utf8mb4`, and numeric/decimal/timestamp/JSON columns decode into their natural JSON types instead of strings.
- **TUI**: `{{ref}}` autocomplete now opens inside HTTP blocks as well (it was DB-only). Typing `{{` lists upstream block aliases (with `cached`/`no result` hint) plus environment variable keys; filters as you keep typing. Same engine used by the SQL completion popup.
- **TUI**: `{{ref}}` placeholders are now highlighted (cyan/bold) in both HTTP and DB block bodies. When the last run failed because of an unresolved ref, that specific ref is painted red inline so the offending alias is visible without reading the status bar.
- **TUI**: running a block now auto-executes any upstream blocks it references that haven't run yet. Diamond chains (B and C both citing A) execute A once; cross-kind chains (HTTP→DB→HTTP) work. Errors abort the chain and surface the failing block; cached upstream blocks are skipped so re-runs only do the necessary work.

### Changed

- **TUI**: vault picker now exposes inline Create / Clone / Open sub-modals via `n` / `c` / `o` chords, replacing the previous `:set-vault <path>` ex-command-only path. The same widgets back the empty-state cards.

- **TUI**: Backspace at a segment boundary (start of a block's body, or start of any segment when the previous one has content) now crosses into the previous segment instead of bailing silently. The buffer behaves like a flat rope: deleting the boundary `\n` merges segments, and if the deletion makes a block's fence stop parsing the block is automatically demoted to plain prose so the renderer shows the text. Undo coalesces a run of cross-boundary deletes into a single step, same as in-segment deletes.
- **TUI**: input routing now goes through an explicit focus stack (`input::scope`). Modals/popups/pickers consume keys by default; unmapped keys never leak to the editor underneath. Replaces a flat priority-ordered chain that allowed Tab and other "universal" actions to fire through an open modal.

### Fixed

- **TUI**: HTTP block method badge no longer offsets the cursor — typing on the URL line previously landed two columns off because the badge rendered wider than the source text.
- **TUI**: keys typed inside an open modal no longer reach the editor behind it (e.g. Tab inside a form switching the editor's tabs).
- **TUI**: arrow keys move the cursor inside the row-detail / response-detail modals when running in standard profile (previously only vim motions were routed there).
- **TUI**: vim visual operators inside the read-only detail modals stay read-only — `va{d` selects but cannot delete; `va{y` still yanks normally.
- **TUI**: opening the `{{` autocomplete inside a block no longer collapses the block from raw view back to its compact display — the popup is a passive overlay so the user keeps typing into the source.
- **TUI**: vim `o` (open below) on the fence closer row now opens a new body line inside the block instead of appending a line outside the fence.
- **TUI**: editing the opening ``` fence into a state that no longer parses (e.g. inserting characters before the backticks) now dissolves the block back to plain prose so the renderer reflects the buffer.
- **TUI**: `{{ref}}` highlight survives the SQL number-token fragmentation — a placeholder like `{{a.response.results.0.rows.0.id}}` now renders as a single cyan span even though the SQL highlighter slices the `0`s as numbers.

## [0.4.0] - 2026-05-18

First public release. httui is a git-native, local-first desktop
markdown editor with executable HTTP and DB blocks inline in
documents and an embedded Claude chat assistant. Vaults are plain
`.md` files plus a `.httui/` sidecar — no proprietary store, no
account.

Distribution: macOS `.dmg` (unsigned), Windows `.msi` / `.exe`,
Linux `.deb` / `.rpm` / AppImage, a Homebrew cask, and a winget
manifest. In-app auto-update is served from GitHub Releases;
pre-releases are opt-in under Settings → General.

### Added

- **Executable HTTP & DB blocks** — run requests and queries inline in markdown documents; chain blocks with `{{ref}}` and the positional `{{$prev.path}}` reference.
- **Claude chat assistant** — embedded assistant with MCP tools, image attachments, and per-session permissions.
- **Git integration** — a collapsible Source Control side panel and a full Git pane-tab: stage / commit, one-click Sync (commit → pull → push), branch switcher, merge-conflict resolution, commit-message templates, history with inline diffs, and share-via-repo URLs.
- **Vault flow** — empty-state Open / Clone / Create cards; a first-run scan prompts for keychain secrets the vault references but the machine lacks.
- **Connections page** — master-detail view with live status, latency, schema preview, and "used in runbooks" navigation.
- **Variables & Environments pages** — a cross-environment variable grid with secret reveal and session overrides, plus environment cards with clone / rename / delete.
- **Quick popovers** — ⌘E environment switcher, connection quick-edit, `{{var}}` inspector, and ⌘⇧V new-variable — none of which steal editor focus.
- **DocHeader** — a card above the editor with the title, abstract, tags, pre-flight checks, and document metadata.
- **Workbench shell** — a new top bar, sidebar (Files / Connections / Variables), and an interactive status bar.
- **File-backed configuration** — connections, environments, and UI preferences live in plain TOML files, with `*.local.toml` overrides and a watcher that picks up external edits.
- **Vault migration** — converts a legacy SQLite vault to the file-based layout (backs up first, idempotent, with a dry-run preview).
- **File conflict detection** — externally modified files surface a banner with Reload / Keep Mine choices.

### Changed

- **HTTP block storage** — request bodies are stored as HTTP-message text inside an `http` fenced code block; legacy JSON-bodied blocks are still parsed on read.
- **Performance** — large response bodies render in a read-only editor instead of blocking the webview, and the HTTP executor caps response bodies at 100 MB.

### Removed

- **TipTap editor and the E2E block** — superseded by CodeMirror 6 and the HTTP block.
- **Top-bar "Run all" button, the editor toolbar, and heading auto-numbering** — dropped as redundant.
- **Web-app and Docker self-host** — explicitly out of scope.

### Fixed

- **Unreadable push errors** — git push rejections (protected branch, non-fast-forward, auth) now show a readable summary instead of raw git stderr.
- **Invisible merge conflicts** — a conflicted vault no longer reports "working tree clean"; the git panel parses unmerged entries and shows the resolver.
- **Markdown round-trip** — `http` and `db-*` fenced blocks survive the CodeMirror parser / serializer cycle without corruption.
- **HTTP headers** — invalid header names produce a clear error instead of a generic `reqwest` builder error.
- **HTTP cancel** — cancelling mid-body returns a clean result rather than partial bytes.
- **Chat auto-save** — auto-save is suppressed while an `update_note` tool call is in flight, driven by tool events instead of timeouts.

### Security

- **Secrets in the OS keychain** — connection passwords and `is_secret` variables are stored in the system keychain, with only a sentinel reference in config files; plaintext fallback applies only when the keychain is unavailable.
- **SQL injection guard** — `{{ref}}` references in SQL blocks are always converted to bind parameters, never string-interpolated.

[Unreleased]: https://github.com/httuicom/httui/compare/v0.4.1...HEAD
[0.4.1]: https://github.com/httuicom/httui/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/httuicom/httui/releases/tag/v0.4.0
