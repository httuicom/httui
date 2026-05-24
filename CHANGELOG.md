# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog 1.1](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Post-0.4.0 work lands here.

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

### Changed

- **TUI**: vault picker now exposes inline Create / Clone / Open sub-modals via `n` / `c` / `o` chords, replacing the previous `:set-vault <path>` ex-command-only path. The same widgets back the empty-state cards.

- **TUI**: Backspace at a segment boundary (start of a block's body, or start of any segment when the previous one has content) now crosses into the previous segment instead of bailing silently. The buffer behaves like a flat rope: deleting the boundary `\n` merges segments, and if the deletion makes a block's fence stop parsing the block is automatically demoted to plain prose so the renderer shows the text. Undo coalesces a run of cross-boundary deletes into a single step, same as in-segment deletes.

## [0.4.0] - 2026-05-18

First public release. httui is a git-native, local-first desktop
markdown editor with executable HTTP and DB blocks inline in documents
and an embedded Claude chat assistant. Vaults are plain `.md` files
plus a `.httui/` sidecar — no proprietary store, no account.

This entry consolidates the foundational storage/secrets work and the
subsequent feature passes: the empty-state vault flow (open / clone /
create), the workbench shell + design system, refined Connections /
Variables / Environments master-detail surfaces, the document
DocHeader (frontmatter + tags), the Git side panel + share-via-repo,
the quick popovers (⌘E / connection chip / `{{var}}` / ⌘⇧V / clone
env), and the unsigned, cross-platform release pipeline.

Distribution: macOS `.dmg` (unsigned developer build — see
`docs/RELEASE.md` for the Gatekeeper bypass), Windows `.msi` / `.exe`,
Linux `.deb` / `.rpm` / AppImage, a Homebrew cask, and a winget
manifest. In-app auto-update is served from GitHub Releases on the
stable channel by default; pre-releases are opt-in under
Settings → General.

### Added

- **⌘E env switcher** — atalho abre um dropdown no seletor de
  ambiente da status bar com atalhos numéricos (1–9) pra trocar de
  env e uma ação rápida "Clone \<env\>".
- **Quick-edit de conexão** — clicar numa conexão na sidebar abre
  um popover com status, "Rotate password", override temporário de
  host:port (só pra sessão, badge TEMPORARY na conexão), Test e
  Duplicate.
- **Popover do `{{var}}` no editor** — clicar num chip `{{var}}`
  mostra o valor no ambiente ativo, um override de sessão e
  "Used in N blocks", sem perder o cursor do editor ao fechar.

- **⌘⇧V nova variável** — popover estilo cmd+K com seletor de tipo
  (Text/Number/Bool/Secret) e helpers de template
  (`{{uuid()}}`, `{{now()}}`, `{{base64()}}`, `{{env()}}`,
  `{{$prev.body.id}}`); salva no ambiente ativo.
- **Referência posicional `{{$prev.path}}`** — encadeia bloco→bloco
  usando a resposta do bloco anterior sem precisar nomear um
  `alias=`.
- **Git side panel (Source Control)** — coluna lateral colapsável
  estilo VS Code, aberta/fechada pelo botão git da top bar e
  persistente entre sessões: status do branch, lista de mudanças
  com stage/unstage por arquivo, campo de commit, Sync, history
  compacto, e botão "Details" pro pane-tab detalhado. Não rouba o
  foco do editor.
- **Sync de 1 clique** — botão Sync faz stage-all → commit → pull
  (fast-forward only) → push numa ação só, com progresso por
  etapa; para na etapa que falhar e mostra o motivo; reusa o
  confirm de set-upstream quando o branch não tem upstream.

- **Template de commit message** — o campo de commit vem
  pré-preenchido (`Update <nota>` / `Update N notes` por padrão);
  configurável em Settings → General com placeholders `{{notes}}`
  / `{{count}}` / `{{date}}`.
- **History compacto + diff inline** — últimos commits no side
  panel (hash, autor, subject, tempo relativo); clicar abre o diff
  do commit ali mesmo; "View all" abre o pane-tab.
- **Faixa de métricas no pane-tab git** — branch, upstream,
  ahead/behind explícito, mudanças por tipo, autor do último
  commit, último sync e URL do remote acima das abas.
- **Git panel** — aba singleton (botão na top bar) com Status / Log:
  working tree (staged/unstaged/untracked), stage/unstage por
  arquivo, commit form com preview, log filtrável (autor / path),
  diff de commit lado-a-lado, push / pull / fetch, e prompt de
  confirmar set-upstream ao dar push numa branch sem upstream.
  Detecta `git remote add` feito por fora sem precisar recarregar.

- **Branch switcher** — o indicador de branch na status bar agora
  abre um picker (branches locais + remotas, filtro, criar nova) que
  faz checkout e recarrega a árvore de arquivos.
- **Resolução de conflito de merge** — banner por arquivo
  conflitado com Accept yours / Accept theirs e um resolvedor 3-way
  (ours editável ↔ theirs, base sob demanda). No editor markdown,
  hunks de conflito ganham destaque (ours/theirs/markers) + ações
  inline na linha do marker (accept current/incoming/both).
- **Share via URL do repositório** — popover (na status bar e no
  git panel) com as URLs HTTPS / SSH / Web do remote; copiar ou
  abrir a Web URL no navegador.
- **DocHeader card acima do CM6** — breadcrumb (workspace › path),
  h1 serif do título, abstract serif, tag chips na coluna direita,
  pill row de pre-flight checks, meta strip com gravatar do owner +
  edited mtime + branch + diff stats (`main +N ~M`) + last run
  status. Card é o ponto de entrada visual da nota.
- **Pre-flight check builder no DocHeader** — `+ Add check` abre
  popover com kind picker (connection / env_var / branch /
  file_exists / command) + CM6 inline editor pra value com
  autocomplete contextual (connections puxam de
  `ConnectionsStore`, env_var da env ativa). Pill cliclável
  abre o mesmo popover em modo edit (pré-bind do kind/value +
  botão Remove).
- **Pre-flight context wiring** — evaluator agora lê o estado real
  do vault: connection names do `ConnectionsStore`, env-var keys
  da env ativa via `EnvironmentsStore`, branch corrente via
  `git rev-parse --abbrev-ref HEAD`. `file_exists` / `command`
  contra FS + PATH. Checks deixam de ser "decorativos" e passam
  a refletir o ambiente.
- **Pre-flight Run-all gate** — `⌘⇧R` (Run all) com pre-flight
  com falha abre dialog de confirmação; `Shift+⌘⇧R` faz override.

- **Variables page (master-detail)** — TopBar `LuKeyRound`
  abre tab dedicada com lista densa cross-env (1 row por chave,
  colunas por env, contagem `USES` via vault-grep), sidebar
  SCOPES/HELPERS, detail panel à direita com value-per-env (Show
  pra secrets resolve via keychain, Edit + Override + Delete por
  row), is_secret toggle com prompt + migração para/do keychain,
  USED IN BLOCKS lista clicável que pula pro arquivo.
- **Session override** — botão `Override` em cada value row salva
  TEMPORARY value em memória (`useSessionOverrideStore`); chip
  `TEMPORARY` clicável dropa. `getActiveVariables` mergeia
  overrides em cima do resolver, então blocos HTTP/DB
  consomem o valor de override. Sem persistência.
- **+ New variable inline form** — table-row style
  (KEY mono input + VALUE input + lock toggle + + save + × cancel)
  inserido no header da Variables page.
- **Environments page** — TopBar `LuLayers` abre tab com cards
  densos por env (varCount, connectionsUsedCount, ACTIVE pill,
  chips personal/temporary). Click ativa o env via
  `set_active_environment`; o pill ACTIVE faz **swap visual
  animado** entre cards (FLIP manual via `getBoundingClientRect`
  + translate inverso, 360ms ease-out).
- **Clone / Rename / Delete environment** — ⋮ menu em cada card
  abre Chakra Popover ancorado embaixo. Clone copia plain vars.
  Rename **migra entries de keychain** (novo backend
  `rename_environment` + `EnvironmentsStore::rename_env`).
  Delete tem banner Destructive + type-the-name confirmation
  (industry-standard guardrail).
- **+ New environment** — Popover anchored no botão pra criar
  novo env (envs/<name>.toml).
- **EnvironmentManager drawer** — refatorado pra consumir
  `VariableValueRow` + `NewVariableForm`. Per-var
  delete shortcut (× vermelho ghost) + + New variable inline
  + per-env Set active / Duplicate / Delete header actions.

- **Master-detail shared atoms** — `components/layout/shared/`
  expõe `SectionLabel`, `SidebarHintCard`,
  `MasterDetailListHeader`, `MasterDetailSidebarRow` +
  constants `MASTER_DETAIL_SIDEBAR_WIDTH` (220px) /
  `_DETAIL_WIDTH` (420px). Connections + Variables agora
  alinham layout pixel-pixel via essas peças.
- **Connections page (master-detail)** — página dedicada (TopBar
  `LuPlug` ou via tab) substitui o drawer legado: lista filtrada por
  kind/env/status com status dot + latência, painel detail com
  credentials + schema preview + "Used in runbooks" (file:line link
  com navegação), modal "New connection" com tabs per-kind
  (Form / Connection string / SSL — SSH placeholder), file picker
  nativo para SQLite db / cert / key paths, ⋮ menu de row
  (Edit / Test / Duplicate / Delete) e config-changed listener
  refletindo edição manual de `connections.toml`.
- **`find_connection_uses_cmd` Tauri command** — vault-grep
  on-demand (`httui_core::connection_uses`) que walk `*.md` e
  retorna `{file, line}` de cada referência `db-<connection>` no
  vault.
- **Workbench shell + design system** — top bar com logo, breadcrumb
  (workspace › project › file), segmented env switcher, ⌘K search e
  branch button substituem a topbar legada. Sidebar nova reúne Files,
  Connections (status dot + latência ms + PROD chip) e Variables
  (lock icon + valor mascarado para entries `is_secret`). Status bar
  interativa expõe env menu, branch menu, contador `+N ~M -D` de
  mudanças git, latência, cursor (Ln/Col), encoding e versão.

- **Inline DocHeader (Notion-mode)** — título serif, abstract, tags
  (chips +/×) e checklist preflight (`[x] item`) editáveis dentro do
  CodeMirror; frontmatter YAML invisível e gerado automaticamente.
  Meta strip do header mostra autor (avatar Gravatar + nome),
  contagem de blocos e last-run inline.
- **Empty-state cards (Open / Clone / Create vault)** — primeiro
  contato com o app sem vault aberto. Three actionable cards
  replace the legacy "Em branco / Templates / Importar" surface,
  with inline error rendering per card and Mac-native directory
  picker. Open and Create rely on `scaffold_new_vault`; Clone
  shells out to `git` and respects the user's credential helper
  / ssh-agent.
- **`clone_vault_cmd` Tauri command** — `git clone <url>
  <parent>/<repo-name>` com leaf derivado da URL e parent
  configurável. Default parent: `~/Documents`. Pre-flight rejeita
  parent inexistente, parent que é arquivo, e leaf não-vazio.
  Backed by `httui_core::git::git_clone`.
- **`create_vault_cmd` Tauri command** — compõe mkdir + `git init`
  + `scaffold_new_vault` numa operação atômica do ponto de vista
  do user. Validações de input rejeitam path traversal (name vazio,
  com `/` ou `\\`, começando com `.`). Backed by
  `httui_core::vault_config::create::create_new_vault`.

- **First-run secrets modal** — quando o vault aberto referencia
  `{{keychain:...}}` ausentes do OS keychain local, abre um modal
  batch após `switchVault`. Cada row tem Save (preenche e remove
  do store) e Skip (esconde da sessão atual mas mantém pendente).
  Skip all / Done dismissam sem tocar o store. Refs ainda pendentes
  ficam visíveis via badge na statusbar (`LuTriangleAlert` +
  contador), clicável para reabrir o modal.
- **`save_secret_cmd` Tauri command** — persiste valor no OS
  keychain. Validações rejeitam `keychain_key` vazio e `value`
  vazio. Driver pra coletar resposta do modal first-run.
- **`make wipe-config`** — limpa estado persistente do app
  (`~/.config/httui`, `~/Library/Application Support/httui`,
  `~/Library/Caches/httui-notes`) sem tocar keychain ou vaults.
  Útil pra dev / debug / voltar pro empty state.
- **File-backed configuration** — connections, environments and the
  per-machine UI prefs now live in plain TOML files (vault root +
  `~/.config/httui/user.toml`), not in `notes.db`. SQLite is retained as
  cache and for ephemeral session state only. (Epics 06–12)
- **Local overrides** — every committed `*.toml` config file accepts a
  sibling `*.local.toml` that deep-merges over the base on read; writes
  always target the base file. The vault's `.gitignore` auto-includes
  the `*.local.toml` block. (see ADR 0004)
- **File watcher** — the desktop app watches `connections.toml`,
  `envs/*.toml`, `.httui/workspace.toml` and `~/.config/httui/user.toml`
  via `notify`; external edits invalidate the in-process cache and
  emit a Tauri event.
- **Vault migration tooling** — Tauri command `migrate_vault_to_v1`
  walks the legacy SQLite tables and writes the v1 file layout. Backs
  up `notes.db` first; idempotent on re-run; supports a dry-run
  preview. (see [`docs/MIGRATION.md`](docs/MIGRATION.md))
- **Secret backend abstraction** — `SecretBackend` trait with a
  `Keychain` default impl plus a parser for `{{keychain:…}}` markers in
  TOML. Slot for future `1Password` / `Stronghold` / `pass` impls.

- **Vault open / scaffold / validate** — `open_vault`,
  `scaffold_vault`, `check_is_vault` Tauri commands; first-run flow
  for empty directories writes the v1 skeleton (`runbooks/`,
  `connections.toml`, `envs/`, `.httui/`, `.gitignore`).
- **First-run missing-secrets scan** — `first_run_missing_secrets`
  Tauri command lists keychain markers referenced by the vault that
  have no value on this machine, so the UI can prompt for batch entry.

- **Settings split foundation** — `user.toml` (per-machine prefs)
  vs. `.httui/workspace.toml` (vault defaults) split, with the seven
  legacy `app_config` UI keys promoted to the new schema. Schema
  bump shipped; UI restructure deferred to a frontend session.
- **Git panel backend** — `httui_core::git` shells out to `git` for
  status, log, branch, fetch, pull, push and remote inspection;
  exposed through Tauri commands ready for the panel UI to consume.

- **Codebase reorganization** — desktop app moved into
  `httui-desktop/`, marketing landing into `httui-web/`, chat sidecar
  into `httui-sidecar/`. Shared logic lives in `httui-core/`. The TUI,
  MCP server and chat sidecar all read the same vault on disk.

- **Quality gates** — pre-push and CI gate every modified `.rs`/`.ts`/
  `.tsx` file at ≤600 production lines and ≥80% line coverage on the
  file as a whole; ESLint warnings for `complexity`,
  `max-lines-per-function`, `max-params`, `max-depth` baseline
  recorded.
- **OSS readiness docs** — README, CONTRIBUTING, SECURITY,
  CODE_OF_CONDUCT, LICENSE plus `docs/ARCHITECTURE.md`, four ADRs
  and user-facing `docs/concepts.md` + `docs/blocks.md`.
  (Epics 01, 36, 37)

### Changed

- **Botão git da top bar** — agora abre/fecha o git side panel
  (antes abria direto o pane-tab). O pane-tab detalhado abre pelo
  "Details" / "View all" dentro do side panel.
- **Design system token vocabulary** — UI 100% alinhada ao
  vocabulário Chakra v3. Tokens custom (`bg.1/2/3/hi`,
  `fg.2/3`, `line`, `line.soft`, `accent.*`, `sel`) foram
  retirados em favor dos defaults Chakra (`bg.subtle/muted/
  emphasized/panel`, `fg.muted/subtle`, `border`, `brand.fg/
  contrast/subtle`). Recipes internos (Menu, Popover, Tooltip,
  Card, Badge) consomem os mesmos nomes — sem slot recipe
  override por componente.
- **File-tree contrast** — items inativos da árvore de arquivos
  passaram de `fg.subtle` para `fg.muted` para garantir
  legibilidade no tema dark Fuji.
- **MarkdownEditor split** — o componente monolítico
  (~573 linhas com `coverage:exclude`) foi quebrado em três
  sub-módulos coesos (`markdown-vim-motions`,
  `markdown-highlight-style`, `markdown-extensions`) com
  100% de cobertura, deixando o shell React em ~206 linhas.
  Comportamento user-visible inalterado.
- **Editor stack** — TipTap rich-text editor and the legacy "E2E"
  block were removed; the editor is now CodeMirror 6 only. Block
  panels (HTTP, DB) mount via React portals into CM6 widget DOM.
- **State management** — most React Contexts replaced by Zustand
  stores (pane, chat, workspace, environment, settings,
  schemaCache). Only `WorkspaceContext` survives.
- **Editor content storage** — moved from React state into a
  module-level `Map` to avoid re-renders on every keystroke; unsaved
  files tracked in a module-level `Set` for the same reason.
- **Performance — large HTTP response bodies** — body viewer is now a
  read-only CodeMirror `EditorView` with language picked from
  `Content-Type`, replacing the older `<pre dangerouslySetInnerHTML>`
  + `lowlight` render that blocked the webview on multi-MB bodies.
- **Performance — HTTP body memory cap** — the executor refuses to
  buffer past 100 MB and returns a `[body_too_large]` placeholder.
- **HTTP block — V1 timing** — `total_ms` + `ttfb_ms` only;
  `dns_ms` / `connect_ms` / `tls_ms` and `connection_reused` deferred
  to V2 (would require swapping `reqwest` for `isahc`/libcurl; see
  `docs/http-timing-isahc-future.md`).
- **HTTP block — fenced-code-native storage format** — body is HTTP
  message text inside a ```http fence (info-string tokens `alias`,
  `timeout`, `display`, `mode`); legacy JSON-bodied blocks are parsed
  on read.

### Removed

- **Aba Audit do git panel** — sem os filtros action-type
  (adiados pra v1.x) era idêntica à aba Log; removida do v1, volta
  com os filtros.
- **Pre-flight `keychain` kind** — retirado do typed set em V6.
  macOS keychain enumeration é restritivo e os call sites que se
  beneficiariam não estão construídos. YAML legado com
  `keychain: <key>` cai pro fallback `Unknown` do parser (não
  crasha — só não aparece como pill).
- **Top bar "Run all" button** — dropado em V2; o roteiro
  inteiro de um documento já é executável bloco-a-bloco e o
  botão acumulava complexidade sem demanda real.
- **EditorToolbar (28 px) acima do CM6** — a faixa duplicava
  o DocHeader (título / branch / "edited just now") e o
  slash command (`/`) já cobre todos os 7 tipos de bloco.
  O componente fica em disco como atom reutilizável, mas
  não é mais montado.
- **Auto-numeração de headings (`# 1.`, `# 1.1`)** —
  removida do editor após validação visual; cabeçalhos
  voltam a ser markdown puro.
- **TipTap-based editor** and its custom vim-mode adapter — replaced
  by CodeMirror 6 with `@replit/codemirror-vim`. (commits 7aa97e8,
  0aa2868, 9124ad4)
- **E2E block type** — superseded by the HTTP block + run-history.
- **Web-app and Docker-self-host roadmap items** — explicitly out of
  scope; marketing landing copy trimmed to match.

### Fixed

- **Erro de push ilegível** — rejeições do push (branch protegida
  / GH013 / non-fast-forward / auth) eram despejadas como o stderr
  cru do git, espremido e ininteligível. Agora vêm com um resumo
  legível em destaque + o detalhe limpo (sem o ruído `remote:`)
  num bloco rolável; o botão volta a "Retry sync".
- **Conflitos de merge invisíveis no git panel** — `git status`
  não interpretava as linhas `u` (unmerged) do `porcelain=v2`, então
  um vault em conflito aparecia como "Working tree clean" e o banner
  de resolução nunca surgia.
- **Markdown serializer round-trip** — fenced code blocks for
  executable types (```http, ```db-*) survive the CM6 markdown
  parser/serializer cycle without corruption.
- **HTTP block — header validity** — invalid HTTP-token header names
  produce a clear error instead of `reqwest`'s generic `builder error`.
- **HTTP block — partial body on cancel** — `tokio::select!` observes
  the cancel token at every chunk in the body loop; cancelling
  mid-body returns a clean `Cancelled` chunk rather than partial bytes.
- **Chat — auto-save vs. MCP writes** — purely event-driven
  suppression of auto-save while a `update_note` tool call is
  in-flight, replacing the earlier timeout-based scheme.
- **File conflict banner** — files modified externally surface a
  banner with Reload / Keep Mine choices; auto-save is suppressed
  while the conflict is unresolved.

### Security

- **Connection passwords** stored in OS keychain by default, with a
  sentinel reference in storage; same applies to environment
  variables marked `is_secret`. Plaintext fallback only when the
  keychain is unavailable.
- **SQL block reference resolution** — `{{alias.response.path}}`
  references in SQL are always converted to bind parameters
  (`$1`, `?`); never string-interpolated. Closes the obvious
  injection vector for chained DB blocks.
- **Touch ID / Windows Hello protection** — design captured in
  Epics 14–15; **not yet shipped** — the implementations are
  blocked on real hardware testing. Until then, the keychain prompt
  in dev/unsigned builds is documented but accepted (see audit-008).

[Unreleased]: https://github.com/httuicom/httui/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/httuicom/httui/releases/tag/v0.4.0
