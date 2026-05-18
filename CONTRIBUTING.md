# Contributing to httui

Thanks for your interest. httui is in pre-v1 — the storage layer, secrets UX, and several flows are actively being reworked, so expect breaking changes on `main`. Bug reports, fixes, docs, and small targeted improvements are all welcome. For larger ideas, please open a discussion or issue first so we can align before the work starts.

## Repo layout

```
httui-core/       Shared Rust crate (executors, DB, references, secrets)
httui-desktop/    Tauri v2 desktop app
  src/            React + TypeScript frontend
  src-tauri/      Rust backend (commands, fs, keychain, sidecar bridge)
httui-tui/        Terminal UI (ratatui)
httui-mcp/        MCP server exposing notes/connections to LLM agents
httui-web/        Marketing landing (deployed to httui.com)
httui-sidecar/    Node.js sidecar for the Claude Agent SDK
```

Cargo workspace at the root drives the Rust crates; npm workspaces drive the JS/TS packages. The desktop app is the integration point: `cargo tauri dev` boots both layers.

## Local setup

Prerequisites:

- Rust stable (1.80+)
- Node.js 20+
- [bun](https://bun.sh) — used by the sidecar build
- Platform deps for Tauri (see the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) page)

Then:

```bash
make install-deps   # cargo fetch + npm install + bun install (sidecar)
make setup-hooks    # symlink the coverage-gate git hooks (see below)
make dev            # boot the desktop app with HMR
```

For the coverage gate (required), also install:

```bash
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov
```

Other useful targets:

```bash
make tui              # run the terminal binary
make sidecar          # rebuild the Node.js sidecar bundle
make test             # cargo workspace + tui tests + vitest unit project
make check            # tsc --noEmit + cargo fmt --check + cargo clippy --all-targets -D warnings
make lint             # eslint across httui-desktop and httui-web
make fmt              # cargo fmt --all + prettier --write across all source dirs
make size-check       # touched-files size gate (≤600 lines per file)
make coverage-check   # touched-files coverage gate (≥80% per file changed)
make quality-check    # size-check + coverage-check (same as pre-push hook)
make coverage-rust    # full HTML coverage report at target/llvm-cov/html
make coverage-fe      # full HTML coverage report at httui-desktop/coverage
make release          # check + test + build — run before tagging a release
make clean            # drop dist/ and target/
```

## Quality gates (must-have)

Every file modified in a commit goes through two gates at push time:

1. **Size gate** — `.rs`/`.ts`/`.tsx` files must stay ≤600 lines of
   production code. (Rust `mod tests { ... }` blocks don't count;
   `*.test.*` and `__tests__/` are skipped.) When a file exceeds the
   limit, the gate prints **"Refactor with SOLID principles"** and
   blocks the push — long files are an SRP smell almost every time.
2. **Coverage gate** — same files must have ≥80% line coverage in the
   file as a whole.

Enforced two ways:

- **Local pre-push hook** (`.git/hooks/pre-push`) blocks the push when
  either gate fails
- **CI** runs the same checks against the PR's base branch

After cloning, run `make setup-hooks` once to wire the hooks in. The
hooks live under `scripts/hooks/` (tracked) and are symlinked into
`.git/hooks/` (untracked).

To inspect what the gate sees locally without trying to push:

```bash
make coverage-check
```

### Escape hatches

Both gates honor file-level opt-outs on line 1:

- `// coverage:exclude file` — skip the coverage check
- `// size:exclude file` — skip the size check

Use sparingly. Justify each opt-out in the PR description, and prefer
splitting the file or mocking heavy deps over taking one. Exclusions
should be reduced over time. Truly atomic files (generated tables,
long but cohesive parser state machines, Tauri command shells with no
logic) are the bar.

## Code style

Rust:

- `cargo fmt --all` is the source of truth — CI enforces it via
  `cargo fmt --all -- --check`. Settings live in `rustfmt.toml`
  (only `edition = "2021"` is pinned; everything else uses rustfmt
  defaults — 100-char line, 4-space indent).
- `cargo clippy --workspace --all-targets -- -D warnings` must pass.
  Project-wide thresholds live in `clippy.toml`. Lint exceptions are
  per-item `#[allow(clippy::...)]` with a comment, never blanket
  disables in the config.

Frontend:

- `prettier` settings in the root `.prettierrc` (semi, double quotes,
  2-space indent, trailing commas).
- ESLint config per workspace (`httui-desktop/eslint.config.js`).
  React Compiler-driven rules (`react-hooks/refs`,
  `react-hooks/set-state-in-effect`, `react-hooks/purity`, etc.) are
  warnings, not errors — they flag intentional patterns in this
  codebase. The two stable rules (`rules-of-hooks`,
  `exhaustive-deps`) stay as errors.

Editor: a `.editorconfig` at the repo root pins line endings (LF),
charset (UTF-8), and indent width per filetype.

## Commit conventions

We use [Conventional Commits](https://www.conventionalcommits.org/). Allowed types:

- `feat` — new user-facing capability
- `fix` — bug fix
- `refactor` — code change with no behavior change
- `chore` — tooling, deps, configs (no production code change)
- `docs` — documentation only
- `test` — tests only
- `style` — formatting / whitespace
- `perf` — performance improvement
- `ci` — CI / workflow / build-pipeline config (used by the
  github-actions Dependabot group)

Scope is optional but encouraged when it disambiguates (`feat(tui):`, `fix(blocks):`, `refactor(http):`). Subject in imperative mood, lowercase, no trailing period. Wrap the body at ~72 columns.

## Branch naming

- `feat/<topic>` — new capability
- `fix/<topic>` — bug fix
- `refactor/<topic>` — internal change
- `docs/<topic>` — docs only
- `chore/<topic>` — tooling

Keep branches short-lived and rebased onto `main`.

## PRs

- Run `make test` and `make check` locally before opening a PR.
- Keep PRs focused on one change. Split refactors out from feature work.
- Reference the issue in the PR body if one exists (`Fixes #N` / `Refs #N`).
- The reviewer will look for: tests where the change introduces or alters logic, no `--no-verify`, no commented-out code, no unrelated formatting churn.
- A CI workflow will be added in a follow-up epic; until then, the local checks above are the gate.

## Where to ask

- **Bugs / regressions** — open a [GitHub issue](https://github.com/httuicom/httui/issues).
- **Feature ideas / design questions** — open a [GitHub discussion](https://github.com/httuicom/httui/discussions).
- **Security issues** — see [SECURITY.md](./SECURITY.md). Do **not** open a public issue.

## Code of Conduct

Participation in this project is governed by the [Code of Conduct](./CODE_OF_CONDUCT.md). By contributing you agree to abide by it.
