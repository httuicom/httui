# httui

**Your API docs, alive.**

A desktop markdown editor with a runtime inside. Write the doc, hit run, ship the proof.

[Website](https://httui.com) · [Releases](https://github.com/httuicom/httui/releases) · [Contributing](./CONTRIBUTING.md) · [Security](./SECURITY.md)

## What is httui?

httui collapses four tools into one markdown file:

| Before | With httui |
|--------|-----------|
| Document APIs in Notion | Docs that execute |
| Test requests in Postman | Requests next to the docs |
| Query the DB in DBeaver | SQL in the same file |
| Chain calls in a shell script | Blocks reference blocks |

Everything serializes to standard `.md` files. Read them in vim, diff them in git, open them in Obsidian.

> See it in action: [httui.com](https://httui.com)

## Features

**Executable blocks** — HTTP requests and SQL queries live inline in your markdown as fenced code blocks.

**Block references** — `{{create-user.response.id}}` lets blocks reference each other. Dependencies execute automatically in the right order. DAG by construction, no cycles possible.

**Database support** — Postgres, MySQL, SQLite with schema-aware autocomplete. SQL references become bind parameters, never string-interpolated.

**Environments** — Key-value variables resolved with `{{KEY}}` syntax. Secret values encrypted via OS keychain.

**AI assistant** — Claude agent with MCP tools that reads, searches, and modifies your notes. Every write stops at a permission prompt with side-by-side diff.

**Multi-pane editor** — Binary tree layout with tabs, drag-drop reordering, split views.

**Vim mode** — CodeMirror 6 with `@replit/codemirror-vim` for full motion support inside fenced blocks.

**Full-text search** — FTS5 index in SQLite. Quick-open (`Cmd+P`) and content search (`Cmd+Shift+F`).

**Result caching** — Results cached by content hash. Rerun only what changed.

## Installation

Pick whichever you prefer — both end up at the same build.

**Install script** (macOS & Linux) — one line, no Homebrew:

```sh
curl -fsSL https://httui.com/install.sh | sh
```

**Homebrew** (macOS & Linux):

```sh
brew tap httuicom/httui
brew install --cask httui
```

**Manual** — download a `.dmg` (macOS), `.msi`/`.exe` (Windows), or
`.deb`/`.rpm`/`.AppImage` (Linux) from
[Releases](https://github.com/httuicom/httui/releases).

> The macOS build is an unsigned developer build. The install script
> and the Homebrew cask both clear the Gatekeeper quarantine for you.
> A manually downloaded `.dmg` needs a one-time
> `xattr -dr com.apple.quarantine /Applications/httui.app`
> (or right-click → Open). In-app auto-update keeps it current after
> the first install.

## 5-minute tour

1. Download the latest release from [Releases](https://github.com/httuicom/httui/releases) and open a folder of markdown files.
2. In any `.md` file, type `/http` to insert a request block:
   ```http alias=octocat
   GET https://api.github.com/users/octocat
   Accept: application/vnd.github+json
   ```
3. Press `▶` (or `Cmd+Enter`) — the response appears inline, with body / headers / cookies / timing tabs.
4. Reference the result in another block:
   ```http
   GET {{octocat.response.html_url}}
   ```
   The dependency runs first; the second request uses the resolved URL.
5. Open a SQL block over a connection (`db-postgres`, `db-mysql`, `db-sqlite`), run queries, reference rows the same way.

That's the loop: write, run, reference, commit. The whole notebook is plain `.md`, so it lives next to the code in your repo.

## Tech stack

| Layer | Technology |
|-------|-----------|
| Runtime | Tauri v2 (Rust) |
| Frontend | React + TypeScript + Vite |
| Editor | CodeMirror 6 |
| UI | Chakra UI v3 + Emotion |
| Storage | SQLite (SQLx) + filesystem (`.md`) |
| Search | FTS5 |
| AI | Claude Agent SDK via Node.js sidecar |
| Secrets | OS keychain (`keyring` crate) |

## Building from source

```bash
# Prerequisites: Rust stable (1.80+), Node.js 20+, bun (for the sidecar)

# Install deps once
make install-deps

# Development (Vite HMR + Tauri rebuild on Rust change)
make dev

# Production build
make build

# Tests
npm test               # Frontend (vitest, runs against httui-desktop)
cargo test --workspace # Backend (all Rust crates)
```

The repo is a Cargo + npm workspace. Top-level layout: `httui-core/`, `httui-desktop/`, `httui-tui/`, `httui-mcp/`, `httui-web/`, `httui-sidecar/`. See [CONTRIBUTING.md](./CONTRIBUTING.md) for the full breakdown.

## Security

- Passwords and secret environment variables are encrypted via the OS keychain. SQLite only stores a sentinel value.
- SQL block references are always converted to bind parameters — zero string interpolation.
- Chat AI writes require explicit user permission with diff review.

Found a security issue? Please follow [SECURITY.md](./SECURITY.md).

## Contributing

Contributions are welcome. Start with [CONTRIBUTING.md](./CONTRIBUTING.md) for local setup, commit conventions, and PR expectations. Please also read the [Code of Conduct](./CODE_OF_CONDUCT.md).

## License

[MIT](./LICENSE) © João Ferreira and httui contributors.
