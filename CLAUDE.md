# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Notes — desktop markdown editor with executable blocks (HTTP client, DB query runner) inline in documents. Built with Tauri v2 (Rust backend) + React + TypeScript + CodeMirror 6 (`@uiw/react-codemirror`) + Chakra UI v3.

> **Repo layout (post epic 00):** the desktop app lives in `httui-desktop/` (`httui-desktop/src/` for the React frontend, `httui-desktop/src-tauri/` for the Rust backend). The marketing landing is `httui-web/`, the Claude sidecar is `httui-sidecar/`. The shared Rust crate is `httui-core/`, the terminal binary `httui-tui/`, the MCP server `httui-mcp/`. **Path references like `src/components/...` in this doc are relative to `httui-desktop/`** unless otherwise prefixed.

> **Recent migrations:** TipTap and the E2E block were removed (commits `7aa97e8`, `0aa2868`, `9124ad4`). The editor is now CodeMirror 6 only. State is managed by Zustand stores, not React Contexts (one legacy context remains: `WorkspaceContext`). Older docs may still reference the old architecture.

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
```

## Empty-state + first-run flow (V1 vertical 1)

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

## Architecture

Full details in `docs/ARCHITECTURE.md` (some sections may be outdated — code is source of truth).

**Block model — aspirational vs actual:**
- *Aspirational*: "plugin architecture (Open/Closed)" — new block types added as vertical slices without modifying existing code, via a `BlockRegistry` and `Executor` trait.
- *Actual*: backend has a real `Executor` trait + dispatch by `block_type` string. Frontend has **no `BlockRegistry`** — block types (HTTP, DB) are imported and wired by hand in `src/components/editor/MarkdownEditor.tsx`. Adding a new block today requires editing `MarkdownEditor.tsx`, creating a CM6 extension under `src/lib/codemirror/`, and adding a Portal mount component under `src/components/editor/`.

**Frontend layers:**
- **CM6 fenced-block extensions** — each block type has a CM6 extension (`src/lib/codemirror/cm-http-block.tsx`, `cm-db-block.tsx`) that scans the doc for its fence (```http, ```db-*), produces decorations with widget DOM containing portal slots (toolbar / form / result / statusbar), and provides a transactionFilter to keep fences atomic-on-edges.
- **Portal mounts** (`src/components/editor/HttpWidgetPortals.tsx`, `DbWidgetPortals.tsx`) subscribe to the CM6 extension's portal registry and `createPortal` the React panels into each slot.
- **Block panels** (`HttpFencedPanel.tsx`, `DbFencedPanel.tsx`) — each is a single large component holding toolbar, form/raw mode, result tabs, status bar, and settings drawer. ⚠️ Both are monoliths (3.876 L and 2.200 L respectively) — pending split. Avoid adding new features inline; prefer extracting sub-components first.
- **`ExecutableBlockShell`** (`src/components/blocks/ExecutableBlockShell.tsx`) — shared shell with display modes (input/split/output), run button, status badge. Currently only consumed by `StandaloneBlock` (the diff-viewer block). HTTP/DB panels reimplement toolbar/status inline because they live outside the editor's document flow (mounted via Portal into CM6 widget DOM).

**Backend layers:**
- `Executor` trait + `ExecutorRegistry` — dispatch by `block_type` string. One generic `execute_block` Tauri command routes to the right executor.
- Tauri `Channel<HttpChunk>` / `Channel<DbChunk>` for real-time streaming from backend to frontend.

**Storage is dual:**
- Vault (filesystem) — `.md` files with executable blocks as fenced code (```http, ```db-*). Plain markdown otherwise.
- SQLite (`notes.db`) — connections, environments, block result cache, app config, schema cache, FTS5 search index, run history, sessions, usage stats.

**SQL safety:** Block references in SQL (`{{alias.response.path}}`) are always converted to bind parameters (`$1`, `?`), never string-interpolated.

**Block references:** `{{alias.response.path}}` — blocks can only reference blocks above them in the document (DAG by construction). Resolution priority: block reference > environment variable (if alias collides with env var, block wins). Environment variables use the same syntax without dots: `{{ENV_KEY}}` resolves from the active environment.

## Key Conventions

- UI components use Chakra UI v3 with Emotion. Use Chakra primitives (Box, Flex, HStack, Menu, Dialog, etc.) and semantic tokens (bg, fg, border). Snippets in `src/components/ui/`. Use `onSelect` (not `onClick`) for `Menu.Item`. Consult the Chakra MCP tools for component examples.
- Do NOT use Chakra `Dialog.Root` for popups that need to return focus to CM6 — use `Portal` + `Box` instead. The Dialog focus trap prevents the editor from receiving keyboard input after closing.
- Tauri IPC uses `invoke()` from `@tauri-apps/api/core`. Frontend wrappers live in `src/lib/tauri/`.
- Passwords and sensitive env variable values are encrypted via OS keychain (`keyring` crate). Sentinel value `__KEYCHAIN__` stored in SQLite, real value in keychain. Fallback to plaintext if keychain unavailable.
- Markdown serialization preserves fenced code blocks for executable blocks (```http, ```db-*) — they must survive roundtrip through the CM6 markdown parser/serializer.

## Performance — critical rules

- **`markUnsaved` must NOT call `setLayout`** — it uses a module-level `Set<string>` (in `src/stores/pane.ts`) to avoid triggering React state updates on every keystroke.
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
- `settings.ts` (140 L) — app config getConfig/setConfig, defaults.
- `schemaCache.ts` (210 L) — DB schema introspection cache, promise dedup for parallel calls on same connection.
- `tauri-bridge.ts` (26 L) — initializes global Tauri event listeners on app start.

### Hooks (`src/hooks/`)

Hooks orchestrate UI flows that wrap stores or plain React state. Many domain hooks of the old design (e.g. `useVault`, `useChat`, `useEnvironments`) became stores.

- `useEditorSession` — file open, auto-save (1s debounce), markdown read/write, suppress/unsuppress auto-save for MCP writes
- `useFileOperations` — CRUD (create/rename/delete/move notes and folders) via Tauri IPC
- `useSessionPersistence` — startup restore + save-on-change via single `restore_session` IPC call
- `useFileSearch` / `useContentSearch` — search modal logic with manual debounce
- `useKeyboardShortcuts` — global Cmd+B/P/S/W/Tab/\ shortcuts
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
- `src/components/layout/connections/` — ConnectionForm, ConnectionsList
- `src/components/layout/environments/` — EnvironmentManager (drawer with env list + key-value editor + secret toggle)
- `src/components/layout/settings/` — settings panels (Audit, Theme, Editor, About)
- `src/components/layout/ConflictBanner.tsx` — banner for externally modified files
- `src/components/layout/TopBar.tsx` — vault selector, environment switcher
- `src/components/chat/` — ChatPanel, ChatConversation, ChatInput, ChatMessageBubble, ChatSessionList, ChatMarkdown, ToolUseGroup, PermissionBanner, PermissionManager, UsagePanel
- `src/components/editor/` — MarkdownEditor (CM6), DiffViewer (side-by-side merge), HttpWidgetPortals, DbWidgetPortals
- `src/components/blocks/` — ExecutableBlockShell, http/fenced/HttpFencedPanel, db/fenced/DbFencedPanel, db/ResultTable, standalone/StandaloneBlock

## Multi-pane system

- Pane layout is a binary tree (`src/types/pane.ts`): each node is either a leaf (tabs + editor) or a split (horizontal/vertical with ratio). Each tab stores its `vaultPath` so tabs from different vaults coexist.
- State managed by `usePaneStore` (`src/stores/pane.ts`). Editor contents stored in module-level `Map` outside Zustand state. Unsaved files tracked in module-level `Set` (not in layout state — avoids re-renders on keystroke).
- Session persistence via `restore_session` Rust command — single IPC call reads all configs, parses layout, reads file contents, and lists workspace in parallel. `list_workspace` filters `node_modules`, `target`, and other heavy directories.

## Vim mode

- Provided by `@replit/codemirror-vim` (CM6 official-ish vim mode). Toggle via StatusBar badge.
- Wired in `MarkdownEditor.tsx` as a CM6 extension.
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
- `src/components/blocks/http/fenced/HttpFencedPanel.tsx` — React panel mounted via `createPortal` into each registered slot. Toolbar (badge / alias / method / host / `[raw│form]` toggle / ▶ / ⚙), result tabs (Body / Headers / Cookies / Timing / Raw with `pretty│raw` sub-toggle), status bar (status dot, host, elapsed, size, "ran X ago", `⤓` Send-as menu), settings drawer (Chakra `Portal` + `Box`, NEVER `Dialog` — preserves CM6 focus). Form mode replaces the body lines with a tabular Params/Headers/Body editor; each input uses local state + commit-on-blur to avoid the round-trip lag of re-emitting raw on every keystroke. **Single file: 3.876 L. Pending split.**
- `src/components/editor/HttpWidgetPortals.tsx` — subscribes to the portal registry and renders panels.

**Execution:**
- Streamed via `executeHttpStreamed` (`src/lib/tauri/streamedExecution.ts`) — `Tauri::Channel<HttpChunk>` carries `Headers { ttfb_ms } → BodyChunk* → Complete`. Frontend uses `onHeaders` for the immediate status update and `onProgress` (cumulative bytes) to drive the "downloading X kb…" status-bar indicator. `Complete` is the cache-write trigger — intermediate `BodyChunk` bytes are discarded by the V1 frontend (the consolidated body lives in `Complete`).
- Cancel via `cancelBlockExecution(executionId)`. The backend's `tokio::select!` observes the token at every chunk in the body loop, so cancel mid-body works (returns `Err("Request cancelled")`, which the Tauri command turns into `HttpChunk::Cancelled`). Partial bytes are discarded.
- Refs `{{...}}` resolved in URL, header keys + values, param keys + values, body before dispatch. Header names that resolve to invalid HTTP tokens (e.g. value with spaces) produce a clear error instead of reqwest's generic `builder error`.
- Cache hash: `sha256(method + URL with sorted-encoded params + sorted headers + body + env-snapshot of *only* referenced vars)`. Mutation methods (POST/PUT/PATCH/DELETE) are NEVER served from cache — they always re-execute.
- Backend executor: `httui-core/src/executor/http/` — `mod.rs` has `HttpExecutor::execute_streamed(params, cancel, on_chunk)` consuming `Response::bytes_stream()` in a loop, and `execute_with_cancel` as a thin wrapper with a no-op callback (so legacy callers keep working unchanged). `types.rs` has `HttpResponse`, `Cookie`, `TimingBreakdown` (with `connection_reused: bool`), `HttpChunk { Headers, BodyChunk, Complete, Error, Cancelled }`. Captures `Set-Cookie` via `parse_set_cookie`.
- **Memory cap:** `MAX_BODY_BYTES = 100 MB`. Above this the executor returns `[body_too_large]` before copying further bytes — defends against OOM on accidental downloads. `is_binary_content_type(content_type)` decides whether `body` is base64-encoded vs JSON-parsed in `Complete`.
- **V1 timing:** `total_ms` (full execution) + `ttfb_ms` (split between `req.send()` returning headers and the first body chunk). `dns_ms`/`connect_ms`/`tls_ms` stay `None` and `connection_reused` stays `false` — the full breakdown requires swapping reqwest for isahc/libcurl, deferred to V2 (see `docs/http-timing-isahc-future.md` for criteria + skeleton).
- **Body viewer:** `HttpBodyCM6Viewer` is a CodeMirror 6 read-only `EditorView` with `oneDarkHighlightStyle` and language picked from Content-Type (`json`/`xml`/`html`/`svg`, with the legacy heuristic as fallback). The `lowlight` package itself stays in `package.json` — still used by `ChatMarkdown`.

**Run history (Story 24.6):** `block_run_history` SQLite table (migration `009`) stores **metadata only** (method, URL canonical, status, sizes, elapsed, outcome, timestamp) — never request/response bodies. Trim: 10 rows per (file_path, alias). Drawer shows last N. Tauri commands: `list_block_history`, `insert_block_history`, `purge_block_history`.

**Code generation (Story 24.7):** `src/lib/blocks/http-codegen.ts` exports `toCurl`, `toFetch`, `toPython`, `toHTTPie`, `toHttpFile`. Snippets are pre-computed in panel state (resolved refs included) so the clipboard write happens synchronously inside the user-gesture window — avoid the gotcha where `await` between click and `clipboard.writeText` silently denies. Status-bar `⤓` menu offers all 5; `Mod-Shift-c` shortcuts directly to cURL.

**Slash commands:** `/HTTP Request`, `/HTTP GET`, `/HTTP POST`, `/HTTP PUT`, `/HTTP DELETE` insert templates in the HTTP-message format with cursor on the request line.

## DB block

- Block type `db-*` (where `*` is the connection id) in `src/components/blocks/db/`. Like the HTTP block, it is a CM6 fenced-code implementation.
- `src/components/blocks/db/fenced/DbFencedPanel.tsx` — React panel (2.200 L, **pending split**). Connection picker, SQL editor, mutation warning for DELETE/UPDATE, result tabs.
- `src/components/blocks/db/ResultTable.tsx` (528 L) — virtualized result grid (`@tanstack/react-virtual`).
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
- **Display mode animation** (`ExecutableBlockShell.tsx`): CSS transitions between input/split/output modes. Used by `StandaloneBlock` (diff viewer); HTTP/DB panels manage modes inline.
- **Mermaid theme sync**: re-initializes with dark/default theme on colorMode change.

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

- `docs/SPEC.md` — Full product specification (features, data models, Tauri commands, UI details). Some references to TipTap/E2E may be stale.
- `docs/ARCHITECTURE.md` — Plugin architecture description (aspirational in places — see Architecture section above).
- `docs/chat-design.md` — Chat system technical design (1000 lines): protocol spec, session lifecycle, streaming, permissions, MCP integration.
- `docs/backlog/` — Epics with stories and tasks. `README.md` has dependency graph and implementation order.

## Compact Instructions

When auto-compacting this conversation, **preserve at all costs**:

- Active epic and story (look at the most recent commit message + `docs-llm/v1/backlog/README.md` for ground truth)
- Quality gate state — whether the last `make quality-check` passed, the threshold (80% / 600 lines), and any active `// size:exclude file` / `// coverage:exclude file` opt-outs
- Recent decisions logged in `docs-llm/jaum-audit/` (autonomous-mode audit trail) — keep the gist; details can be re-read
- The Definition-of-Done rules (`docs-llm/v1/definition-of-done.md`) — non-negotiable
- The `docs-llm/v1/out-of-scope.md` list — never touch these
- For autonomous mode (`/auto-start`): the loop discipline — decide-and-audit, never ask, never push to remote, never bypass gates by editing scripts

You can drop:
- Verbose tool output (cargo build/test stdout, file listings)
- Earlier exploration of files that are now well-understood
- Old plan-mode discussions that already resulted in committed code
- Step-by-step user prompts/agreements once the action is reflected in commits

When in doubt, re-read the active epic file from `docs-llm/v1/backlog/` rather than relying on summarized memory of it.
