# Getting started

This page walks you from a fresh httui install to running your
first HTTP and SQL blocks. ~10 minutes.

If you want the conceptual overview first (what's a vault, how do
environments work, what's a block), [start with `concepts.md`](./concepts.md)
and come back.

## Install

httui is a desktop app (Tauri) plus a terminal binary (TUI).
Pre-built installers ship per release; you can also build from
source.

### From a release (recommended once v1.0 ships)

| Platform | Bundle |
|---|---|
| macOS (Apple Silicon) | DMG — `httui-aarch64-apple-darwin.dmg` |
| macOS (Intel) | DMG — `httui-x86_64-apple-darwin.dmg` |
| Linux | AppImage + `.deb` |
| Windows | MSI |

Download from the [GitHub Releases page](https://github.com/httuicom/httui/releases),
double-click to install. The DMG and MSI ship code-signed once
Epic 34 (developer cert work) lands; until then macOS may show
a Gatekeeper prompt and Windows SmartScreen may warn on first
install — both are safe to dismiss.

Homebrew (`brew install httui`) and winget submissions are
upcoming with v1.0; track progress in Epic 35 Story 04.

### From source

You'll need:

- Rust stable (`rustup`)
- Node 20+ (`nvm`)
- [Bun](https://bun.sh/) (for the chat sidecar build)
- macOS / Linux / Windows with the Tauri prereqs:
  <https://v2.tauri.app/start/prerequisites/>

```bash
git clone https://github.com/httuicom/httui
cd httui
make install-deps
make build              # production .app on macOS
# or
make dev                # hot-reload dev mode (Vite + Tauri rebuild)
```

The dev build opens at `http://localhost:1420` — Tauri proxies
the webview at that address. The release build lands at
`target/release/bundle/<platform>/httui.<ext>`.

## Open, clone, or create a vault

When httui boots without an active vault it shows three options:

- **Open** — pick an existing folder. If it has a `.httui/`,
  `runbooks/`, `connections.toml`, or top-level `.md` file, httui
  treats it as a vault and activates it.
- **Clone** — paste a git remote URL. httui clones it locally
  and opens. (Git operations route through the same `httui-core`
  git module the in-app panel uses — Epic 48 owns the panel UI.)
- **Create** — pick an empty folder. httui scaffolds the
  directory layout: `runbooks/`, `connections.toml`,
  `envs/local.toml`, `.httui/workspace.toml`, and a `.gitignore`
  that hides `*.local.toml` overrides + `notes.db`.

Either way, the active vault is now your working directory. Files
you save land in the vault root; the editor lives at the center
of the workbench.

## Your first runbook

Create a new `.md` file under `runbooks/` (right-click the file
tree, "New note"). Paste:

````markdown
# Smoke test

A simple runbook to verify the install works.

```http alias=ping timeout=5000
GET https://httpbin.org/json
Accept: application/json
```
````

Click the ▶ button on the block toolbar (or `Cmd+Enter` while the
cursor is in the body). The response renders below the block:

- **Body** tab — pretty-printed JSON
- **Headers** tab — response headers
- **Timing** tab — total / ttfb breakdown
- **Raw** tab — wire format

The `alias=ping` token names the block; the next block can
reference its response with `{{ping.body.slideshow.title}}`.
Block references are resolved before send — see
[concepts.md → references](./concepts.md) for the full syntax.

## Add a connection, run a SQL block

The HTTP block is built-in. SQL blocks need a registered
connection — that lives in `connections.toml`. The easiest path
is the in-app form:

1. Click **Connections** in the sidebar (database icon).
2. **Add connection** → fill the form. For a quick sanity check:
   - Type: SQLite
   - Name: `local`
   - Path: `./scratch.sqlite` (httui creates the file on first run)
3. Save. The form writes `connections.toml`:
   ```toml
   [connections.local]
   type = "sqlite"
   path = "./scratch.sqlite"
   ```

Now in your runbook, add a DB block:

````markdown
```db-local
CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    email TEXT
);

INSERT INTO users (name, email)
VALUES ('Alice', 'alice@example.com');
```
````

Click ▶. The first run of a mutation shows a confirmation banner
(DELETE/UPDATE/INSERT prompt for confirmation by default — see
the per-block settings if you want to skip the prompt for trusted
runbooks).

Add a SELECT block to read it back:

````markdown
```db-local
SELECT * FROM users;
```
````

The result shows in a virtualized grid (sortable columns, type
badges per column). The first row of every result also feeds the
chained-block reference — `{{prev.row[0].id}}` resolves to `1`
for the next block.

## Connect a real Postgres / MySQL

Same form, swap the type. For Postgres:

```toml
[connections.payments-staging]
type = "postgres"
host = "pg-staging.acme.local"
port = 5432
database = "payments"
user = "{{keychain:payments-staging:user}}"
password = "{{keychain:payments-staging:password}}"
ssl_mode = "require"
```

The `{{keychain:...}}` references are filled in at run time from
the OS keychain (Keychain on macOS, Credential Manager on
Windows, Secret Service on Linux). The form prompts you for the
raw values once and stores them; the TOML never holds plaintext.

If the keychain is locked or the entry is missing, httui shows
a "Resolve secrets" banner before the next run — see
[`secrets`](./concepts.md#secrets) in the concepts doc.

## Multi-environment workflow

Your local Postgres lives on `127.0.0.1:5432` but the team's
shared `connections.toml` points at `pg-staging.acme.local:5432`.
Drop a sibling file:

```toml
# connections.local.toml — gitignored, per-machine override
[connections.payments-staging]
host = "127.0.0.1"
port = 15432
```

Now your runs hit the SSH tunnel, but the committed file still
points at the canonical address. Your teammate's
`connections.local.toml` can have a different shape; both
machines run the same runbook with their respective tunnels.

The same pattern works for `envs/<name>.local.toml` — useful for
local-only env vars without polluting the committed env file.

## What next

- **Add tags + frontmatter to your runbooks** —
  [`docs/blocks.md`](./blocks.md) has the full fence syntax.
- **Use the chat panel** — Cmd+B toggles it. The chat reads and
  writes notes via MCP tools, so you can ask "summarize the
  payments-staging runbook" or "rerun the smoke test and tell me
  if anything changed since yesterday".
- **Open the same vault from another machine** — git push +
  pull. Connections, env metadata, and runbooks travel; secrets
  and per-machine overrides stay where they belong.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| "Vault not recognized" on Open | Folder has none of `.httui/`, `runbooks/`, top-level `.md`, `connections.toml`, or `envs/` | Create one of them, or use the "Create" flow to scaffold. |
| Connection test fails with `Failed to connect to <driver>` | Driver not installed / DB unreachable / wrong host | Check `connections.toml` against the team's shared file; verify the `.local` override matches your local tunnel. |
| Block shows "missing secret" | Keychain doesn't have the entry yet | Run the first-run secret setup from the connections panel, or store via the OS keychain UI. |
| `make dev` hangs at "compiling" | Tauri prereqs missing | Install the Tauri prereqs for your OS (link in the From-source section above). |
