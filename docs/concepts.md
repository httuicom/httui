# Concepts

> If you've worked in a notebook editor before (Jupyter, Hex, Mode,
> Postman collections), most of this will feel familiar — except
> that everything is a file in a git repo, and httui has no server.

## Vault

A **vault** is just a git repository. Inside it:

- `runbooks/` holds your `.md` files
- `connections.toml` defines the databases / HTTP targets your
  blocks talk to
- `envs/<name>.toml` holds per-environment variables
- `.httui/workspace.toml` holds workspace-shared defaults
- `notes.db` (SQLite) caches results, run history, and chat
  sessions — gitignored

When you "open" a vault, httui scans it for the above files. If
none are present it offers to scaffold them.

## Environments

Switching env is reading a different file. No branches, no
checkouts, no working-tree changes.

```
envs/local.toml         # your local-dev URLs and tokens
envs/staging.toml       # shared staging config (committed)
envs/staging.local.toml # your personal override (gitignored)
envs/prod.toml          # production config (branch-protected)
```

The TopBar dropdown lists every `envs/*.toml` (skipping `.local`
siblings). Switching the active env is purely a read change — your
runbook content doesn't move.

### Personal overrides

Drop `envs/staging.local.toml` next to the committed
`envs/staging.toml` to override individual values for yourself
without touching the shared file:

```toml
# envs/staging.local.toml — gitignored
[vars]
BASE_URL = "http://localhost:8080"
```

httui deep-merges the override on top of the base at read time.
**Writes from the app always target the base file** (committed),
never the `.local` sibling. The override stays your private
side-channel.

The same pattern applies to `connections.local.toml` and
`.httui/workspace.local.toml`.

## Secrets

Sensitive values live in the OS keychain, never in the TOML file.
The TOML carries a reference instead:

```toml
[connections.payments-staging]
type = "postgres"
host = "pg-staging.acme.local"
user = "app"
password = "{{keychain:conn:payments-staging:password}}"
```

When you create a connection through the app, httui:

1. Stores the password in the keychain under
   `conn:<connection-id>:password`
2. Writes only the `{{keychain:...}}` reference to
   `connections.toml`

A teammate cloning the same vault gets the **reference**, not
your password. They run a runbook, the app sees the ref hasn't
been populated locally, and the first-run modal prompts them to
fill it in.

### Reference syntax

Anywhere a TOML value would normally hold a string, you can drop
in a `{{backend:address}}` reference:

| Backend | Example | Meaning |
|---|---|---|
| `keychain` | `{{keychain:conn:pg:password}}` | OS keychain entry |
| `1password` | `{{1password:op://Personal/db/password}}` | 1Password CLI lookup (planned) |
| `pass` | `{{pass:databases/staging}}` | passwordstore.org / GPG agent (planned) |
| `env` | `{{env:DB_URL}}` | OS env var (escape hatch) |

### Anti-cleartext check

httui's TOML validator **rejects raw secret values** in
`[secrets]` sections. If you write `password = "hunter2"` directly
the app refuses to save and asks you to use a reference instead.
The escape hatch is a `# httui:allow-cleartext` comment on the
preceding line — only useful for genuinely non-sensitive values
that happen to share the field name.

## References inside runbooks

Block bodies can reference values from earlier blocks plus
environment variables:

```http
GET {{BASE_URL}}/users/{{$prev.body.id}}
Authorization: Bearer {{ADMIN_TOKEN}}
```

- `{{BASE_URL}}` — variable from the active env
- `{{ADMIN_TOKEN}}` — secret from the active env (resolves through
  the keychain)
- `{{$prev.body.id}}` — captured value from the previous block's
  response

The reference resolver walks the document **above the current
block** to find named outputs. Block aliases (`alias=req1` in the
fence info string) make captures explicit:

```http
```http alias=login
POST {{BASE_URL}}/auth/login
Content-Type: application/json

{ "user": "admin" }
```

Then in the next block:

```http
GET {{BASE_URL}}/me
Authorization: Bearer {{login.body.token}}
```

## Pre-flight checks

A runbook can declare preconditions in YAML frontmatter. httui
evaluates them when you open the file and again before **Run all**;
failures surface as red pills in the DocHeader card.

```markdown
---
title: "Refund a payment"
preflight:
  - connection: payments-db
  - env_var: STRIPE_KEY
  - branch: main
  - keychain: payments-db.password
  - file_exists: ./schema/refund.sql
  - command: psql --version
---

Body of the runbook starts here…
```

The six check kinds:

| Kind | Resolves against | Pass when |
|---|---|---|
| `connection` | `connections.toml` | the named alias exists |
| `env_var` | active env (`envs/<name>.toml` + `.local` overrides) | the key is set (any value) |
| `branch` | the vault's git repo | the current branch name matches |
| `keychain` | OS keychain (or active backend) | the entry exists (presence only, no value read) |
| `file_exists` | the vault root | the path resolves to an existing file (relative paths resolve against the vault; absolute paths pass through) |
| `command` | `$PATH` | the binary is found (path-qualified commands check the literal path) |

Each item produces one of three outcomes:

- **Pass** — green ✓ pill.
- **Fail** — red ✗ pill with a reason ("connection `payments-db`
  not found", "on branch `feature/x`, expected `main`"). Click the
  pill to jump to the panel that fixes it.
- **Skip** — grey – pill. Used when the check can't be evaluated
  (e.g. `branch:` in a non-git vault) or when the kind is unknown
  to this httui version (forward-compat).

### Run-all gate

When you click **Run all** with one or more failing checks, httui
blocks with a confirmation: "*N pre-flight checks failed. Run
anyway?*" Hold **Shift** while clicking to skip the gate; the
override is recorded in the run-all report ("3 failed pre-flight,
ran anyway via shift") so reviewers can spot it later.

The pill row also has a manual **Re-check** button — useful after
you switch envs or land a fix in another window.

### When to use it

- New teammate runbooks: declare what the runbook needs so a fresh
  clone fails *visibly* instead of mid-execution.
- Production playbooks: pin `branch: main` to refuse to run from a
  feature branch by accident.
- Domain-specific tooling: `command: psql` confirms the local
  binary is installed before a SQL block tries to shell out.

## Sharing a vault

httui has no "share modal", "snapshot link", or expiring-password
flow. **Sharing a vault is sharing the git repo.** Every collaborator
who clones the repo gets the same TOML config and runbooks; secrets
stay in each person's local keychain (per the [Secrets](#secrets)
section).

The git panel offers three one-click actions over the configured
`origin` remote:

| Action | What you copy / open |
|---|---|
| **Copy repo URL** | `https://github.com/acme/runbooks` (or whatever the active remote points at — picker if you have multiple) |
| **Copy permalink at current commit** | `https://github.com/acme/runbooks/blob/<sha>/runbooks/payments-debug.md#L42` — pinned to the current HEAD sha so the link doesn't drift if the file changes later |
| **Open pull request** | Opens `https://github.com/acme/runbooks/compare/main...feature/x` in the browser via `tauri.shell.open` |

Hosts auto-detected from the remote URL: GitHub, GitLab.com,
self-hosted GitLab (any `gitlab.*` host), Bitbucket, Gitea. The first
three get the right URL shape (`/blob/`, `/-/blob/`, `/compare/`);
Bitbucket and Gitea fall back to "open the remote in your browser"
plus a copy-URL button — adding shape parsers for them is a later
follow-up, not a blocker.

If the vault doesn't have an `origin` remote, the popover reports
"No remote configured" and links into Workspace settings to add one.

## Multi-user, multi-machine

The git workflow does the heavy lifting:

- **Two devs, one vault**: edits propagate through normal
  pull-request flow. Each dev keeps their personal `.local.toml`
  overrides + their own keychain.
- **One dev, two machines**: code/runbooks sync via git. Secrets
  are re-entered per machine — intentional, secrets never leave
  the box. Power users point at 1Password / pass to skip this
  prompt (planned).

## What's a file vs SQLite

| Data | Lives in | Synced via git |
|---|---|---|
| Runbooks (`.md`) | repo | yes |
| Connections / envs / workspace defaults | committed `*.toml` | yes |
| Passwords + secret env vars | OS keychain | no |
| Personal overrides | `*.local.toml` | no (gitignored) |
| Per-machine prefs (theme, font) | `~/.config/httui/user.toml` | no |
| Run history | SQLite | no |
| Block result cache | SQLite | no |
| Chat sessions | SQLite | no |

SQLite is **cache + ephemeral state**, not source of truth.

## See also

- [Block authoring](./blocks.md) — fence syntax, references,
  capture/chain
- [`docs/ARCHITECTURE.md`](./ARCHITECTURE.md) — code shape and
  process model
- [ADR 0001 — TOML schemas](./adr/0001-toml-schemas.md)
- [ADR 0002 — Secret references](./adr/0002-secret-references.md)
- [ADR 0004 — Local overrides](./adr/0004-local-overrides.md)
