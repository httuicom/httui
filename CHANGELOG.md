# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog 1.1](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Post-0.4.1 work lands here.

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
