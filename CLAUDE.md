# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Notes — desktop markdown editor with executable blocks (HTTP client, DB query runner) inline in documents. Built with Tauri v2 (Rust backend) + React + TypeScript + CodeMirror 6 (`@uiw/react-codemirror`) + Chakra UI v3.

> **Repo layout:** the desktop app lives in `httui-desktop/` (`httui-desktop/src/` for the React frontend, `httui-desktop/src-tauri/` for the Rust backend). The Claude sidecar is `httui-sidecar/`. The shared Rust crate is `httui-core/`, the terminal binary `httui-tui/`, the MCP server `httui-mcp/`. **Path references like `src/components/...` in this doc are relative to `httui-desktop/`** unless otherwise prefixed. The marketing landing and end-user docs live in a separate repo, [`httuicom/httui-site`](https://github.com/httuicom/httui-site) (deployed to [httui.com](https://httui.com)) — they are intentionally out of this repo to keep AI context lean.

> **Recent migrations:** TipTap and the E2E block were removed (commits `7aa97e8`, `0aa2868`, `9124ad4`). The editor is now CodeMirror 6 only. State is managed by Zustand stores, not React Contexts (the only legacy *domain* context left is `WorkspaceContext`; small CM6/UI-scoped contexts like `BlockContext` and the doc-header context also exist by design). Older docs may still reference the old architecture.

## Commands

```bash
# Development
make dev                           # Run app in dev mode (frontend HMR + backend rebuild)
npm run dev                        # Frontend only (Vite dev server)

# Build
cargo tauri build                  # Production build
npm run build                      # Frontend production build

# Backend tests
cargo test --workspace             # Run all Rust tests across all crates
cargo test -p httui-notes <name>   # Run specific test in tauri crate

# Frontend tests
npm run test                       # Run all frontend tests (vitest)
npm run test -- <pattern>          # Run specific test
npm run test:coverage              # Run with v8 coverage report (HTML at coverage/index.html)

# Lint
npm run lint                       # ESLint
cargo clippy --workspace           # Rust linter (all crates)

# Dev utilities
make wipe-config                   # Apaga config persistente do app (notes.db, user.toml, WebKit cache).
                                   # Mantém keychain. Útil pra voltar ao empty state entre testes manuais.
make setup-hooks                   # Install local git hooks (pre-commit / pre-push / commit-msg).

# TUI / launcher (post-install — `make install && make install-tui` on macOS,
# or via the cask/.deb/.rpm)
httui                              # opens the TUI (terminal mode)
httui desktop                      # opens the desktop app
httui --help
```

> **Bundle layout:** the release ships three binaries together —
> `httui` (launcher on the PATH), `httui-tui` (terminal binary, was
> `notes-tui`), `httui-desktop` (Tauri main, the one Finder runs).
> The launcher dispatches to a sibling via `current_exe()`. See
> `httui-launcher/`, `scripts/prepare-bundle-bins.sh`, and
> `docs/RELEASE.md §7a`.

## Commit style

Enforced by `scripts/hooks/commit-msg` (installed via `make setup-hooks`):

- Subject only — NO body, NO `-m "..."` follow-ups for "context".
- Conventional Commits subject (`feat(tui): ...`, `refactor: ...`).
- No internal planning vocabulary: `V<n>`, `tui-V<n>`, `vertical[- ]<n>`,
  `slice`, `fase`/`phase`, `cenario`/`cenário`, `p<n>`. Those live in
  `docs-llm/`, not in `git log`.
- No AI-assistant attribution: `Generated with`, `Co-Authored-By: Claude`, 🤖.
- Subject ≤ 72 chars.

Bypass with `--no-verify` only with explicit owner approval. Iterative
fix commits should be squashed (`git reset --soft HEAD~N` + re-commit).

## Empty-state + first-run flow

Mounted in `AppShell` when `vaultPath === null`:

- `EmptyVaultScreen` — three cards: **Open** (file picker → `switchVault`),
  **Clone** (form → `clone_vault_cmd` → derived `<parent>/<repo-name>` →
  `switchVault`), **Create** (form → `create_vault_cmd` → mkdir + `git
  init` + `scaffold_new_vault` → `switchVault`).
- After `switchVault`, `usePendingSecretsScan` invokes
  `list_missing_secrets`. If non-empty, the `PendingSecretsModal` opens
  with a Save/Skip per row. Skipped refs stay in the store so the
  `StatusBar` badge surfaces them; clicking the badge re-opens the
  modal. `save_secret_cmd` persists each one to the OS keychain.
- Tauri wrappers for these flows live in `src/lib/tauri/vault-ops.ts`
  (`cloneVault`, `createVault`, `saveSecret`), re-exported from
  `commands.ts`. Backend modules: `httui-core::git::clone`,
  `httui-core::vault_config::create`, `vault_config_commands.rs`.
- **TUI parity:** `httui-tui` has the same empty-state on first run —
  `httui-tui/src/empty_state/` owns its own alt screen + event loop and
  reuses the existing `ui/vault_*` widgets plus
  `vault::helpers::{submit_create, submit_clone, read_dir_entries}`.
  Pending-secrets modal + status-bar badge live in the same code paths
  as the in-app vault picker (`Alt+K` → `n`/`c`/`o`).

## Architecture

Full architecture write-up lives at [httui.com/docs/architecture](https://httui.com/docs/architecture) (some sections may be outdated — code is source of truth).

**Block model — OCP closed (refactor 2026-05-19/20, audit 03 #2):**
- *Public contract*: `BlockTypeSpec` (`src/lib/blocks/block-registry.ts`)
  + `BlockPortalEntry` (`src/lib/blocks/block-portal-registry.tsx`).
  Adding a new block type today = create `cm-<id>-block.tsx` + a
  `<Id>FencedPanel.tsx`, then append a `BlockTypeSpec` to
  `blockRegistry` and a `BlockPortalEntry` to `blockPortals`. **Zero
  edits** to `MarkdownEditor.tsx`, `markdown-extensions.ts`, or
  `cm-slash-commands.ts` — they consume the arrays.
- *Backend*: `Executor` trait + `ExecutorRegistry` dispatch by
  `block_type` string. One generic `execute_block` Tauri command routes
  to the right executor.

**Frontend layers:**
- **CM6 fenced-block extensions** — each block type has a CM6 extension
  (`src/lib/codemirror/cm-http-block.tsx`, `cm-db-block.tsx`) built on
  the shared generic skeleton in `createFencedBlockExtension.tsx` +
  `widget-portal-registry.ts` (scanner, registry, decorations, keymap,
  StateField, ref autocomplete are all generic; each block file owns
  only its open-fence regex, body decoration, and any block-specific
  fields like DB's error squiggle).
- **Generic portal mount** — `src/components/editor/BlockWidgetPortals.tsx`
  (80 L). One component instantiated twice in `MarkdownEditor.tsx`
  (once per block type via the `blockPortals` array). Replaces the
  pre-A4 `HttpWidgetPortals.tsx` / `DbWidgetPortals.tsx`.
- **Block panels** (`HttpFencedPanel.tsx` 567 L, `DbFencedPanel.tsx`
  779 L) — orchestrators that mount React panels into the per-block
  CM6 widget slots via `createPortal`. The HTTP panel was decomposed
  into 4 sibling hooks (`useHttpRefsContext`, `useHttpCacheHydrate`,
  `useHttpCodegenSnippets`, `useHttpDrawerData`) + 4 view siblings
  (`HttpBodyView`, `HttpInlineEditors`, `HttpFormTables`,
  `HttpJsonVisualizer`) + a pure helper module
  (`http-request-builder.ts`). All files now under the 600-line size
  gate. DB panel split is deferred (no audit-#-blocker remaining).
- **`StandaloneBlockShell`** (`src/components/blocks/StandaloneBlockShell.tsx`,
  ex-`ExecutableBlockShell` renamed in A6) — shared shell with display
  modes (input/split/output), run button, status badge. Currently only
  consumed by `StandaloneBlock` (the diff-viewer block). HTTP/DB
  panels reimplement toolbar/status inline because they live outside
  the editor's document flow (mounted via Portal into CM6 widget DOM).
- **Shared FSM hook** — `src/hooks/useExecutableBlock.ts` (A2a) owns
  the `idle → running → success | error | cancelled` machine + the
  AbortController + collect-blocks/env + try/catch/finally. The HTTP
  panel consumes it via an adapter (`validate`, `prepare`, `execute`,
  `elapsedOf`, `persist?`, `onOutcome?`); the DB panel kept its
  in-place reducer (its `runExplain` / `loadMore` co-mutate the same
  state — decision documented in audit 03 #4 RULE 4).

**Backend layers:**
- `Executor` trait + `ExecutorRegistry` — dispatch by `block_type` string. One generic `execute_block` Tauri command routes to the right executor.
- Tauri `Channel<HttpChunk>` / `Channel<DbChunk>` for real-time streaming from backend to frontend.

**Storage is dual:**
- Vault (filesystem) — `.md` files with executable blocks as fenced code (```http, ```db-*). Plain markdown otherwise.
- SQLite (`notes.db`) — connections, environments, block result cache, app config, schema cache, FTS5 search index, run history, sessions, usage stats.

**SQL safety:** Block references in SQL (`{{alias.response.path}}`) are always converted to bind parameters (`$1`, `?`), never string-interpolated.

**Block references:** `{{alias.response.path}}` — blocks can only reference blocks above them in the document (DAG by construction). Resolution priority: block reference > environment variable (if alias collides with env var, block wins). Environment variables use the same syntax without dots: `{{ENV_KEY}}` resolves from the active environment. `{{$prev.path}}` (V11) is a positional alias: the previous executed block, response as the implicit root (so `{{$prev.body.id}}` ≈ `<prev>.response.body.id`) — no explicit `alias=` needed. Resolves only (hover tooltips inherit it); `$prev` is not in the `{{` autocomplete (deferred v1.x).

## Key Conventions

- UI components use Chakra UI v3 with Emotion. Use Chakra primitives (Box, Flex, HStack, Menu, Dialog, etc.) and semantic tokens (bg, fg, border). Snippets in `src/components/ui/`. Use `onSelect` (not `onClick`) for `Menu.Item`. Consult the Chakra MCP tools for component examples.
- Do NOT use Chakra `Dialog.Root` for popups that need to return focus to CM6 — use `Portal` + `Box` instead. The Dialog focus trap prevents the editor from receiving keyboard input after closing.
- Tauri IPC uses `invoke()` from `@tauri-apps/api/core`. Frontend wrappers live in `src/lib/tauri/`.
- Passwords and sensitive env variable values are encrypted via OS keychain (`keyring` crate). Sentinel value `__KEYCHAIN__` stored in SQLite, real value in keychain. Fallback to plaintext if keychain unavailable.
- Markdown serialization preserves fenced code blocks for executable blocks (```http, ```db-*) — they must survive roundtrip through the CM6 markdown parser/serializer.

## Performance — critical rules

- **`markUnsaved` must NOT call `setLayout`** — it uses a module-level `Set<string>` (in `src/stores/pane.ts`) to avoid triggering React state updates on every keystroke. To observe "a save just landed" reactively, subscribe to the `saveSignal` counter (bumped by `notifySaved` after each `writeNote` resolves) instead of trying to flip a derived `dirty` prop.
- **Editor content store is a module-level `Map`** (in `src/stores/pane.ts`) — mutated in place by `updateContent`. This is intentional non-reactive state to avoid re-renders on every keystroke. Don't move it into Zustand state.
- **CSS objects passed to the editor must be static** (extracted outside the component as constants) to avoid Emotion recomputation on re-render.
- **Body viewers** (`HttpBodyCM6Viewer` in `HttpFencedPanel.tsx`) use a read-only CodeMirror `EditorView` with language picked from Content-Type — replaced an older `<pre dangerouslySetInnerHTML>` + `lowlight` render that blocked the webview on multi-MB bodies.

## Frontend architecture

**`AppShell`** is a thin composition layer that wires Zustand stores and the surviving `WorkspaceContext`.

### Stores (`src/stores/`)

State is centralized in Zustand stores. Test pattern: call `useStore.getState()` / `setState()` directly, with `beforeEach` resetting state. See `src/hooks/__tests__/usePaneState.test.ts` for the convention.

- `pane.ts` (435 L) — pane layout binary tree, tab management (file + diff tabs), module-level editor content `Map` and unsaved files `Set`, file conflict resolution, `forceReloadFile` action.
- `chat.ts` (559 L) — chat state machine: messages, streaming deltas, tool activity grouping, permission queue, MCP integration. Listens to Tauri events `chat:delta`, `chat:done`, `chat:error`, `chat:tool_use`, `chat:tool_result`, `chat:permission_request`.
- `workspace.ts` (133 L) — vault path, file tree, switchVault, openVault, listens to `connection-status` Tauri events.
- `environment.ts` (109 L) — environment + variable CRUD, active environment, `is_secret` keychain shim.
- `settings.ts` (~190 L) — app config getConfig/setConfig, defaults; also owns persisted UI prefs incl. `gitSidePanelOpen` + `gitCommitTemplate` (user.toml `[ui]`).
- `git.ts` (V10.1) — single source of truth for git: polled status/remotes/commits + commit draft + `lastSyncAt`. Refcounted 2s poll via `acquire`/`release`. `useGitStatus`/`useGitRemotes` are store-backed shims (signature preserved). Test reset: `resetGitStore()` (called in global `src/test/setup.ts`).
- `schemaCache.ts` (210 L) — DB schema introspection cache, promise dedup for parallel calls on same connection.
- `tauri-bridge.ts` (26 L) — initializes global Tauri event listeners on app start.
- `envSwitcher.ts` / `newVariablePopover.ts` (V11) — tiny UI stores bridging the ⌘E / ⌘⇧V shortcuts (wired in AppShell) to their popovers (`EnvSwitcher` / `NewVariablePopover`). In-memory only.
- `connectionSessionOverride.ts` (V11) — session-only `{host?,port?}` per connection id (mirrors `sessionOverride.ts`). Applied per DB run by `applyConnectionOverride` in `lib/tauri/streamedExecution.ts` → backend `PoolManager.get_pool_with_override` (override-keyed pool; base pool untouched). Never persisted.

### Hooks (`src/hooks/`)

Hooks orchestrate UI flows that wrap stores or plain React state. Many domain hooks of the old design (e.g. `useVault`, `useChat`, `useEnvironments`) became stores.

- `useEditorSession` — file open, auto-save (1s debounce), markdown read/write, suppress/unsuppress auto-save for MCP writes
- `useFileOperations` — CRUD (create/rename/delete/move notes and folders) via Tauri IPC
- `useSessionPersistence` — startup restore + save-on-change via single `restore_session` IPC call
- `useFileSearch` / `useContentSearch` — search modal logic with manual debounce
- `useKeyboardShortcuts` — global Cmd+B/P/S/W/Tab/\ shortcuts; also ⌘E (`openEnvSwitcher`) + ⌘⇧V (`openNewVariable`) wired in AppShell (V11)
- `useSidebarResize` — drag-to-resize sidebar
- `useEscapeClose` — generic escape-to-close hook
- `useStickyScroll` — DOM scroll behavior
- `usePromptDialog` — Chakra Dialog wrapper for prompts
- `useTheme` — wrapper for `useColorMode()`
- `useAutoUpdate` — Tauri updater plugin orchestration

### Contexts (`src/contexts/`)

Only one survives:
- `WorkspaceContext` (29 L) — wires `fileOps` callbacks for AppShell consumption. Most domain contexts (PaneContext, ChatContext, EnvironmentContext, etc.) were replaced by stores.

### Component structure

- `src/components/layout/file-tree/` — FileTree (with @dnd-kit drag-drop), FileTreeNode, InlineInput
- `src/components/layout/pane/` — PaneContainer, PaneNode, SplitView
- `src/components/layout/connections/` — `ConnectionsPage` master-detail (rendered as a singleton `TabState.kind = "connections"` pane tab opened from the TopBar `LuPlug` button) plus the legacy sidebar `ConnectionsList` + drawer `ConnectionForm` (kept — popover quick-edit in V11 builds on top of the sidebar)
- `src/components/layout/variables/` — `VariablesPage` (singleton `TabState.kind = "variables"` pane tab, TopBar `LuKeyRound`); cross-env merge container with detail panel (per-env value rows + secret reveal + session override + USED IN BLOCKS + is_secret toggle); inline NewVariableForm.
- `src/components/layout/environments/` — `EnvironmentsPage` (singleton `TabState.kind = "environments"` pane tab, TopBar `LuLayers`) with cards per env, Clone/Rename/Delete via ⋮ menu in a Chakra `Popover` anchored to the source card (virtual `getAnchorRect`); + New environment as anchored Popover on the header button; ACTIVE pill animates between cards via manual FLIP. `EnvironmentManager` drawer (640px right Portal) is the quick-edit alternative — same `useEnvironmentStore` + `setVariable` / `deleteVariable` + per-row `VariableValueRow` (with `keyLabel`).
- `src/components/layout/shared/` — master-detail atoms (`SectionLabel`, `SidebarHintCard`, `MasterDetailListHeader`, `MasterDetailSidebarRow`) + width constants shared by Connections + Variables.
- `src/components/layout/environments/` — EnvironmentManager (drawer with env list + key-value editor + secret toggle)
- `src/components/layout/settings/` — settings panels (Audit, Theme, Editor, About)
- `src/components/layout/ConflictBanner.tsx` — banner for externally modified files
- `src/components/layout/git/` — Git, two complementary surfaces over the shared `useGitStore` (V10.1):
  - **Side panel** (`GitSidePanel`, V10.1): right collapsible column (Box, NOT Dialog — preserves CM6 focus), mounted in `AppShell` like ChatPanel; open state persisted (`useSettingsStore.gitSidePanelOpen`). The TopBar `LuGitBranch` button toggles it. Composes `GitStatusHeader` + `GitFileList` + `GitCommitForm` + `GitSyncBar` + `GitSidePanelHistory` + a "Details" button → pane-tab. Sub-components extracted for SRP: `GitSyncBar`, `GitSidePanelHistory`, `GitMetricsStrip`, `GitCommitTemplateField`.
  - **Pane-tab**: `GitPanelContainer` (data/dispatch) → `GitPanel` (Status/Log; `GitMetricsStrip` band on top) composing carry sub-components (GitStatusHeader/GitFileList/GitCommitForm/GitLogList/GitLogFilter/GitCommitDiffViewer/GitBranchPicker/GitSyncButtons/GitConflictBanner/GitConflictResolver). Still a `SingletonTabKind = "git"` pane-tab; opened from the side panel's Details/View-all (not the TopBar button anymore).
  - **Shared hooks** (single source — both surfaces): `useGitCommit`, `useGitStage`, `useGitSync` (stage-all → commit → pull `--ff-only` → push). Commit-message prefill: `lib/blocks/commit-template.ts` (default + `{{notes}}/{{count}}/{{date}}`). Push-error formatting: `lib/blocks/git-error.ts`.
  - `ShareMenu` (status bar + panel toolbar) wraps `share/SharePopover` via `useShareRepoUrl`. Branch switcher lives in `BranchMenu` (status bar). Conflict regions in the markdown editor are decorated by `src/lib/codemirror/cm-merge-conflict.tsx`. Backend: `httui-core/src/git/` (`conflict.rs` = `git show :1|:2|:3`; `git_push` has `set_upstream`; `git_pull` has `ff_only`).
- `src/components/layout/TopBar.tsx` — vault selector, environment switcher
- `src/components/chat/` — ChatPanel, ChatConversation, ChatInput, ChatMessageBubble, ChatSessionList, ChatMarkdown, ToolUseGroup, PermissionBanner, PermissionManager, UsagePanel
- `src/components/editor/` — MarkdownEditor (CM6 composition shell, ~223L), DiffViewer (side-by-side merge), BlockWidgetPortals (generic, 80L — replaces pre-A4 HttpWidgetPortals/DbWidgetPortals), DocHeaderWidgetPortal. The CM6 extension stack lives in three sibling modules with 100% coverage: `markdown-vim-motions.ts` (vim compartment + doc-line `ArrowUp/Down` keymap + `moveByLines` motion override), `markdown-highlight-style.ts` (Chakra-driven `HighlightStyle` + `dbSqlLanguages` + `containerCss`), and `markdown-extensions.ts` (`buildExtensions(params)` + `flattenFiles` helper) — the last consumes `blockRegistry` from `src/lib/blocks/block-registry.ts` so adding a block type doesn't require editing this file.
- `src/components/blocks/` — StandaloneBlockShell, http/fenced/HttpFencedPanel (+ 4 sibling hooks + 4 view siblings + http-request-builder), db/fenced/DbFencedPanel, db/ResultTable, standalone/StandaloneBlock

## Multi-pane system

- Pane layout is a binary tree (`src/types/pane.ts`): each node is either a leaf (tabs + editor) or a split (horizontal/vertical with ratio). Each tab stores its `vaultPath` so tabs from different vaults coexist.
- State managed by `usePaneStore` (`src/stores/pane.ts`). Editor contents stored in module-level `Map` outside Zustand state. Unsaved files tracked in module-level `Set` (not in layout state — avoids re-renders on keystroke).
- Session persistence via `restore_session` Rust command — single IPC call reads all configs, parses layout, reads file contents, and lists workspace in parallel. `list_workspace` filters `node_modules`, `target`, and other heavy directories.

## Vim mode

- Provided by `@replit/codemirror-vim` (CM6 official-ish vim mode). Toggle via StatusBar badge.
- Wired in `MarkdownEditor.tsx` as a CM6 extension; the compartment, the doc-line `ArrowUp/Down` keymap and the `moveByLines` motion override live in `markdown-vim-motions.ts` and are imported by the shell.
- The previous custom TipTap-based vim implementation under `src/components/editor/vim/` was removed.

## Search

- Quick-open (`Cmd+P`): fuzzy file name search via Rust `search_files` with subsequence scoring.
- Full-text (`Cmd+Shift+F`): FTS5 index in SQLite, rebuilt on vault switch, `search_content` with snippet highlighting.
- Both use Portal-based panels (not Dialog) to avoid focus trap issues.

## HTTP block

The HTTP block is a fenced-code-native CM6 implementation (epic 24 — `docs/http-block-redesign.md`). Methods: GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS.

**Storage format** — body is HTTP-message text inside a ```http fence:
```
```http alias=req1 timeout=30000 display=split mode=raw
GET https://api.example.com/users?page=1
Authorization: Bearer {{TOKEN}}
```
```
Info-string tokens: `alias`, `timeout`, `display`, `mode` (`raw|form`). Canonical write order is `alias → timeout → display → mode`. Pre-redesign blocks with a JSON body (`{"method":"...","url":"..."}`) are detected by the parser and converted on read — vault stays compatible.

**Architecture:**
- `src/lib/blocks/http-fence.ts` — parser/serializer for both info string and HTTP-message body. `parseHttpMessageBody` / `stringifyHttpMessageBody` are idempotent (canonical reformat). `parseLegacyHttpBody` + `legacyToHttpMessage` handle the JSON shim.
- `src/lib/codemirror/cm-http-block.tsx` — CM6 extension: scanner, decorations, atomic-on-fences-only, transactionFilter, method coloring on the first body line, keymap (⌘↵ run, ⌘. cancel, ⌘⇧C copy as cURL). Holds a portal registry (toolbar / form / result / statusbar slots) so React mounts inside the widget DOM.
- `src/components/blocks/http/fenced/HttpFencedPanel.tsx` (567 L) — React orchestrator panel mounted via `createPortal` into each registered slot. Toolbar (badge / alias / method / host / `[raw│form]` toggle / ▶ / ⚙), result tabs (Body / Headers / Cookies / Timing / Raw with `pretty│raw` sub-toggle), status bar (status dot, host, elapsed, size, "ran X ago", `⤓` Send-as menu), settings drawer (Chakra `Portal` + `Box`, NEVER `Dialog` — preserves CM6 focus). Form mode replaces the body lines with a tabular Params/Headers/Body editor; each input uses local state + commit-on-blur to avoid the round-trip lag of re-emitting raw on every keystroke. **Decomposed (A1 + follow-ups, 2026-05):** orchestrator delegates to sibling hooks (`useHttpRefsContext`, `useHttpCacheHydrate`, `useHttpCodegenSnippets`, `useHttpDrawerData`) + view siblings (`HttpToolbar`, `HttpStatusBar`, `HttpResultTabs`, `HttpFormMode`, `HttpSettingsDrawer`, `HttpBodyView`, `HttpInlineEditors`, `HttpFormTables`, `HttpJsonVisualizer`) + the pure `http-request-builder.ts` helpers. All under the 600-line size gate; 4 hooks at 100% cov + adapter callbacks tested at 81.8%.
- `src/components/editor/BlockWidgetPortals.tsx` (80 L, generic) — subscribes to the portal registry and renders the panel for that block type. Instantiated twice in `MarkdownEditor.tsx` via the `blockPortals` array (one entry per block type).

**Execution:**
- Streamed via `executeHttpStreamed` (`src/lib/tauri/streamedExecution.ts`) — `Tauri::Channel<HttpChunk>` carries `Headers { ttfb_ms } → BodyChunk* → Complete`. Frontend uses `onHeaders` for the immediate status update and `onProgress` (cumulative bytes) to drive the "downloading X kb…" status-bar indicator. `Complete` is the cache-write trigger — intermediate `BodyChunk` bytes are discarded by the V1 frontend (the consolidated body lives in `Complete`).
- Cancel via `cancelBlockExecution(executionId)`. The backend's `tokio::select!` observes the token at every chunk in the body loop, so cancel mid-body works (returns `Err("Request cancelled")`, which the Tauri command turns into `HttpChunk::Cancelled`). Partial bytes are discarded.
- Refs `{{...}}` resolved in URL, header keys + values, param keys + values, body before dispatch. Header names that resolve to invalid HTTP tokens (e.g. value with spaces) produce a clear error instead of reqwest's generic `builder error`.
- Cache hash: `sha256(method + URL with sorted-encoded params + sorted headers + body + env-snapshot of *only* referenced vars)`. Mutation methods (POST/PUT/PATCH/DELETE) are NEVER served from cache — they always re-execute.
- Backend executor: `httui-core/src/executor/http/` — `mod.rs` has `HttpExecutor::execute_streamed(params, cancel, on_chunk)` consuming `Response::bytes_stream()` in a loop, and `execute_with_cancel` as a thin wrapper with a no-op callback (so legacy callers keep working unchanged). `types.rs` has `HttpResponse`, `Cookie`, `TimingBreakdown` (with `connection_reused: bool`), `HttpChunk { Headers, BodyChunk, Complete, Error, Cancelled }`. Captures `Set-Cookie` via `parse_set_cookie`.
- **Memory cap:** `MAX_BODY_BYTES = 100 MB`. Above this the executor returns `[body_too_large]` before copying further bytes — defends against OOM on accidental downloads. `is_binary_content_type(content_type)` decides whether `body` is base64-encoded vs JSON-parsed in `Complete`.
- **V1 timing:** `total_ms` (full execution) + `ttfb_ms` (split between `req.send()` returning headers and the first body chunk). `dns_ms`/`connect_ms`/`tls_ms` stay `None` and `connection_reused` stays `false` — the full breakdown requires swapping reqwest for isahc/libcurl, deferred to V2 (see `docs/http-timing-isahc-future.md` for criteria + skeleton).
- **Body viewer:** `HttpBodyCM6Viewer` is a CodeMirror 6 read-only `EditorView` with `oneDarkHighlightStyle` and language picked from Content-Type (`json`/`xml`/`html`/`svg`, with the legacy heuristic as fallback). The `lowlight` package itself stays in `package.json` — still used by `ChatMarkdown`.

**Run history:** `block_run_history` SQLite table (migration `009`) stores **metadata only** (method, URL canonical, status, sizes, elapsed, outcome, timestamp) — never request/response bodies. Trim: 10 rows per (file_path, alias). Drawer shows last N. Tauri commands: `list_block_history`, `insert_block_history`, `purge_block_history`.

**Code generation:** `src/lib/blocks/http-codegen.ts` exports `toCurl`, `toFetch`, `toPython`, `toHTTPie`, `toHttpFile`. Snippets are pre-computed in panel state (resolved refs included) so the clipboard write happens synchronously inside the user-gesture window — avoid the gotcha where `await` between click and `clipboard.writeText` silently denies. Status-bar `⤓` menu offers all 5; `Mod-Shift-c` shortcuts directly to cURL.

**Slash commands:** `/HTTP Request`, `/HTTP GET`, `/HTTP POST`, `/HTTP PUT`, `/HTTP DELETE` insert templates in the HTTP-message format with cursor on the request line.

## DB block

- Block type `db-*` (where `*` is the connection id) in `src/components/blocks/db/`. Like the HTTP block, it is a CM6 fenced-code implementation.
- `src/components/blocks/db/fenced/DbFencedPanel.tsx` (779 L) — React panel. Connection picker, SQL editor, mutation warning for DELETE/UPDATE, result tabs. Decomposition decision (audit 03 #4): the panel's `runExplain` / `loadMore` co-mutate the same FSM state as `run`, so it stays in-place rather than consume `useExecutableBlock` (RULE 4 — would require exposing setters and lose encapsulation). Adapter-based reuse would distort the contract.
- `src/components/blocks/db/ResultTable.tsx` (525 L) — virtualized result grid (`@tanstack/react-virtual`).
- Streamed via `executeDbStreamed` (`src/lib/tauri/streamedExecution.ts`).
- SQL safety: `{{...}}` references are converted to bind parameters (`$1`, `?`) before dispatch — never string-interpolated.

## Environments

- Managed via `useEnvironmentStore` (`src/stores/environment.ts`). Tables `environments` and `env_variables` in SQLite.
- TopBar dropdown to select active environment. EnvironmentManager drawer (`src/components/layout/environments/`) for CRUD + key-value editing.
- `{{KEY}}` (no dots) in any HTTP/DB block field resolves to the active environment's variable value. Keys appear in `{{` autocomplete alongside block aliases.
- Backend: 8 Tauri commands for full CRUD (list/create/delete/duplicate environments, set active, list/set/delete variables).
- Sensitive variables: `is_secret` flag + lock toggle in UI. Secret values encrypted via OS keychain (`keyring` crate), sentinel `__KEYCHAIN__` in SQLite.

## Security — Keychain

- Module: `httui-desktop/src-tauri/src/db/keychain.rs` — `store_secret`, `get_secret`, `delete_secret`, `resolve_value`.
- Connection passwords: stored in keychain on create/update, sentinel in SQLite. Resolved in `build_connection_string`.
- Environment variables: `is_secret` field (migration `002_env_is_secret.sql`). Secret values stored in keychain, resolved on read in `row_to_variable`.
- Fallback: if keychain unavailable, values stored plaintext with no error.

## Block utilities

Shared infrastructure in `src/lib/blocks/`:
- `references.ts` — parse `{{...}}` syntax, resolve against block contexts + env variables, navigate JSON by dot-path. Priority: block ref > env var.
- `dependencies.ts` — extract referenced aliases, auto-execute dependencies before current block. Dedup lock via `inflightExecutions` Map prevents duplicate execution of shared dependencies.
- `cm-references.ts` (in `src/lib/codemirror/`) — CodeMirror decoration plugin for `{{ref}}` syntax highlighting + hover tooltip showing resolved values or errors.
- `cm-autocomplete.ts` (in `src/lib/codemirror/`) — CodeMirror completion for `{{` — shows block aliases (with cached/no result detail) and env variable keys (with env detail).
- `hash.ts` — SHA-256 content hash for block result cache invalidation.
- `document.ts` — walk CM6 doc to collect blocks above current position.

Test coverage is high (~95%) for everything in `src/lib/blocks/` — see `src/lib/blocks/__tests__/`.

## Editor features

- **File conflict banner** (`src/components/layout/ConflictBanner.tsx`): shown when an open file is modified externally. Options: Reload (re-read from disk) or Keep Mine (overwrite). Auto-save suppressed during conflict.
- **Display mode animation** (`StandaloneBlockShell.tsx`): CSS transitions between input/split/output modes. Used by `StandaloneBlock` (diff viewer); HTTP/DB panels manage modes inline.
- **Mermaid theme sync**: re-initializes with dark/default theme on colorMode change.
- **Inline `{{ref}}` popover** (V11): `lib/blocks/cm-ref-popover.ts` (pure `handleRefMousedown` + emitter + `refClickExtension`, wired in `markdown-extensions.ts`) → `RefPopoverHost` mounts `RefPopover` via Chakra `Popover.Root` + virtual `getAnchorRect` (NOT Dialog; `autoFocus=false` + `onOpenChange→closeRefPopover` restores caret/CM6 focus). All V11 popovers (EnvSwitcher, ConnectionQuickEdit, RefPopover, NewVariablePopover) use Chakra `Popover.Root`/Portal — no `Dialog.Root`.

## Chat system

- Full design in `docs/chat-design.md`. Chat panel in `src/components/chat/`. State lives in `src/stores/chat.ts`.
- Architecture: React frontend → Tauri Rust backend → Node.js sidecar (`httui-sidecar/src/`) → Claude Agent SDK. Communication via NDJSON protocol over stdin/stdout.
- Sidecar spawned lazily on first chat message. Health-checked via ping/pong. Auto-respawn with exponential backoff.
- MCP server: `httui-mcp` binary with 14 tools (list/read/create/update notes, search, connections, environments). Registered as MCP tool for the sidecar.

**Sessions:** SQLite-backed (`sessions` table). `claude_session_id` for resume across restarts. On resume failure, offers "Continue as new conversation" (clears `claude_session_id`, re-sends last message).

**Permission system:** `PermissionBroker` (`httui-desktop/src-tauri/src/chat/permissions.rs`) intercepts tool calls before prompting the user. Cascading logic:
1. Bash → always ask user
2. Edit/Write outside session `cwd` → hard deny (no prompt)
3. Read/Glob/Grep inside session `cwd` → auto-allow
4. DB persisted rule (`tool_permissions` table, scope `always`) → apply
5. DB session rule (scope `session`) → apply
6. Fallback → ask user via PermissionBanner

PermissionBanner (`src/components/chat/PermissionBanner.tsx`): scope selector (Once/Session/Always). For `update_note` tools, shows compact banner with file path, line stats (+N -M), and "View Diff" button. PermissionManager panel (gear icon) lists and deletes persisted rules.

**Diff viewer:** When `update_note` is detected, opens a side-by-side diff tab (`src/components/editor/DiffViewer.tsx`) using `@codemirror/merge`. Both sides read-only. Fenced code blocks (```http, ```db-*) rendered as executable `StandaloneBlock` widgets inside CodeMirror via `StateField` decorations (`src/lib/codemirror/cm-block-widgets.tsx`). Blocks have SQL/JSON syntax highlighting (`oneDarkHighlightStyle`) and line-level diff decorations (red for deletions, green for additions). Allow/Deny buttons in diff header.

**Diff tab lifecycle:** `TabState` extended with `kind: "diff"`. `usePaneStore` has `openDiffTab`/`closeDiffTab` actions. Diff tabs are transient — filtered from session persistence.

**Auto-save protection for MCP writes:** Event-driven state machine in chat store:
- `chat:tool_use` with `update_note` → `onFileWriteStart` callback → `suppressAutoSave(filePath)` (cancels pending auto-save timer)
- `chat:tool_result` for that tool → `onFileWriteComplete` callback → `unsuppressAutoSave(filePath)` + `forceReloadFile(filePath)` (reloads from disk into editor)
- No timeouts — purely driven by tool lifecycle events.

**Image attachments:** File picker, clipboard paste, and Tauri native drag-drop (`getCurrentWebview().onDragDropEvent()`). Max 20 images, 5MB each. Images normalized before sending to Claude: resize if either side > 2048px (Lanczos3), re-encode as JPEG Q85 (`normalize_image` in `commands.rs`, uses `image` crate).

**CWD per session:** Displayed in chat header bar (truncated path). Click to change via directory picker. Falls back to active vault path. Passed to sidecar for tool execution context.

**Wikilinks in chat:** User text scanned for `[[target]]` patterns in `send_chat_message`. Matching notes resolved by filesystem search (case-insensitive stem match). Note content injected as context blocks for the sidecar. Original `[[...]]` preserved in DB for display.

**Usage stats:** Tokens aggregated per day/session in `usage_stats` table (upserted on `chat:done`). `cache_read_tokens` tracked alongside `input_tokens`/`output_tokens`. UsagePanel (`src/components/chat/UsagePanel.tsx`) shows CSS bar chart (last 30 days), cache efficiency percentage, and summary cards. Accessible via "Usage" tab in ChatPanel.

## Testing

- **Framework:** Vitest 4 with two projects — `unit` (jsdom) and `browser` (Playwright via `@vitest/browser-playwright`).
- **Coverage:** `npm run test:coverage` runs the unit project with v8 coverage. HTML report at `coverage/index.html`.
- **Mocks:** Tauri IPC is mocked via `src/test/mocks/tauri.ts` (configurable handler registry — `mockTauriCommand(cmd, handler)` / `clearTauriMocks()`). Tauri events stubbed in `src/test/mocks/tauri-event.ts`.
- **Conventions:**
  - Zustand stores: test directly via `useStore.getState()` / `setState()` with `beforeEach` reset (see `src/hooks/__tests__/usePaneState.test.ts`).
  - React hooks: `renderHook` from `@testing-library/react`.
  - Components: `render` + `screen` + `userEvent.setup()`. Always `clearTauriMocks()` in `afterEach`.
  - Pure logic (parsers, references, codegen): plain function tests in `src/lib/blocks/__tests__/`.
  - Browser-only tests use the suffix `.browser.test.tsx` (e.g. `cm-scroll.browser.test.tsx`).

## Docs

- **Public end-user docs:** [httui.com/docs](https://httui.com/docs) (source: [`httuicom/httui-site`](https://github.com/httuicom/httui-site)).
- **Internal / AI-only references:** `docs-llm/` (gitignored; spec, architecture deep-dives, chat-design, audits).

## Compact Instructions

When auto-compacting this conversation, **preserve at all costs**:

- The current task/goal and any in-progress work not yet committed
- Quality gate state — whether the last `make quality-check` passed and the active threshold (80% coverage / 600 lines per touched file)
- Decisions made this session and their rationale (keep the gist; details can be re-read from the commits)
- Any non-negotiable constraints the user has stated

You can drop:
- Verbose tool output (cargo build/test stdout, file listings)
- Earlier exploration of files that are now well-understood
- Old plan-mode discussions that already resulted in committed code
- Step-by-step user prompts/agreements once the action is reflected in commits

When in doubt, prefer re-reading the current code and commits over relying on summarized memory.
