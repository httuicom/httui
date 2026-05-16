# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog 1.1](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased] — pre-v1

httui has not been publicly released yet. The codebase on `main` is being
reworked toward v1; expect breaking changes between commits. The first
tagged release will be `v1.0.0`.

The list below tracks notable changes accumulated during the v1
foundation work (epics 00–37). The "v1 launch" line is reached once the
remaining items in the
[Definition of Done](docs-llm/v1/backlog/README.md#definition-of-done--v1)
checklist are green — primarily the React frontend cutover (Epic 19),
the signed/cross-platform release pipeline (Epics 34–35), and the final
launch checklist (Epic 38, Story 03).

### Added

- **Git side panel (Source Control)** — coluna lateral colapsável
  estilo VS Code, aberta/fechada pelo botão git da top bar e
  persistente entre sessões: status do branch, lista de mudanças
  com stage/unstage por arquivo, campo de commit, Sync, history
  compacto, e botão "Details" pro pane-tab detalhado. Não rouba o
  foco do editor. (V10.1 cenários 1, 5)
- **Sync de 1 clique** — botão Sync faz stage-all → commit → pull
  (fast-forward only) → push numa ação só, com progresso por
  etapa; para na etapa que falhar e mostra o motivo; reusa o
  confirm de set-upstream quando o branch não tem upstream.
  (V10.1 cenário 3)
- **Template de commit message** — o campo de commit vem
  pré-preenchido (`Update <nota>` / `Update N notes` por padrão);
  configurável em Settings → General com placeholders `{{notes}}`
  / `{{count}}` / `{{date}}`. (V10.1 cenários 2, 8)
- **History compacto + diff inline** — últimos commits no side
  panel (hash, autor, subject, tempo relativo); clicar abre o diff
  do commit ali mesmo; "View all" abre o pane-tab. (V10.1 cenário 4)
- **Faixa de métricas no pane-tab git** — branch, upstream,
  ahead/behind explícito, mudanças por tipo, autor do último
  commit, último sync e URL do remote acima das abas. (V10.1
  cenário 6)
- **Git panel** — aba singleton (botão na top bar) com Status / Log:
  working tree (staged/unstaged/untracked), stage/unstage por
  arquivo, commit form com preview, log filtrável (autor / path),
  diff de commit lado-a-lado, push / pull / fetch, e prompt de
  confirmar set-upstream ao dar push numa branch sem upstream.
  Detecta `git remote add` feito por fora sem precisar recarregar.
  (V10 cenários 1-5, 8)
- **Branch switcher** — o indicador de branch na status bar agora
  abre um picker (branches locais + remotas, filtro, criar nova) que
  faz checkout e recarrega a árvore de arquivos. (V10 cenário 4)
- **Resolução de conflito de merge** — banner por arquivo
  conflitado com Accept yours / Accept theirs e um resolvedor 3-way
  (ours editável ↔ theirs, base sob demanda). No editor markdown,
  hunks de conflito ganham destaque (ours/theirs/markers) + ações
  inline na linha do marker (accept current/incoming/both). (V10
  cenário 6)
- **Share via URL do repositório** — popover (na status bar e no
  git panel) com as URLs HTTPS / SSH / Web do remote; copiar ou
  abrir a Web URL no navegador. (V10 cenário 7)
- **DocHeader card acima do CM6** — breadcrumb (workspace › path),
  h1 serif do título, abstract serif, tag chips na coluna direita,
  pill row de pre-flight checks, meta strip com gravatar do owner +
  edited mtime + branch + diff stats (`main +N ~M`) + last run
  status. Card é o ponto de entrada visual da nota. (V6 cenários 1-2)
- **Pre-flight check builder no DocHeader** — `+ Add check` abre
  popover com kind picker (connection / env_var / branch /
  file_exists / command) + CM6 inline editor pra value com
  autocomplete contextual (connections puxam de
  `ConnectionsStore`, env_var da env ativa). Pill cliclável
  abre o mesmo popover em modo edit (pré-bind do kind/value +
  botão Remove). (V6 cenário 9)
- **Pre-flight context wiring** — evaluator agora lê o estado real
  do vault: connection names do `ConnectionsStore`, env-var keys
  da env ativa via `EnvironmentsStore`, branch corrente via
  `git rev-parse --abbrev-ref HEAD`. `file_exists` / `command`
  contra FS + PATH. Checks deixam de ser "decorativos" e passam
  a refletir o ambiente. (V6 cenário 9 — `def8e27`, `3c3be69`)
- **Pre-flight Run-all gate** — `⌘⇧R` (Run all) com pre-flight
  com falha abre dialog de confirmação; `Shift+⌘⇧R` faz override.
  (V6 cenário 10)
- **Variables page (master-detail)** — TopBar `LuKeyRound`
  abre tab dedicada com lista densa cross-env (1 row por chave,
  colunas por env, contagem `USES` via vault-grep), sidebar
  SCOPES/HELPERS, detail panel à direita com value-per-env (Show
  pra secrets resolve via keychain, Edit + Override + Delete por
  row), is_secret toggle com prompt + migração para/do keychain,
  USED IN BLOCKS lista clicável que pula pro arquivo. (V5
  cenários 1-4)
- **Session override** — botão `Override` em cada value row salva
  TEMPORARY value em memória (`useSessionOverrideStore`); chip
  `TEMPORARY` clicável dropa. `getActiveVariables` mergeia
  overrides em cima do resolver, então blocos HTTP/DB
  consomem o valor de override. Sem persistência. (V5 cenário 3)
- **+ New variable inline form** — table-row style
  (KEY mono input + VALUE input + lock toggle + + save + × cancel)
  inserido no header da Variables page. (V5 cenário 5)
- **Environments page** — TopBar `LuLayers` abre tab com cards
  densos por env (varCount, connectionsUsedCount, ACTIVE pill,
  chips personal/temporary). Click ativa o env via
  `set_active_environment`; o pill ACTIVE faz **swap visual
  animado** entre cards (FLIP manual via `getBoundingClientRect`
  + translate inverso, 360ms ease-out). (V5 cenário 6)
- **Clone / Rename / Delete environment** — ⋮ menu em cada card
  abre Chakra Popover ancorado embaixo. Clone copia plain vars.
  Rename **migra entries de keychain** (novo backend
  `rename_environment` + `EnvironmentsStore::rename_env`).
  Delete tem banner Destructive + type-the-name confirmation
  (industry-standard guardrail). (V5 cenários 7-8)
- **+ New environment** — Popover anchored no botão pra criar
  novo env (envs/<name>.toml). (V5 cenário 6 polish)
- **EnvironmentManager drawer** — refatorado pra consumir
  `VariableValueRow` + `NewVariableForm` (V5 atoms). Per-var
  delete shortcut (× vermelho ghost) + + New variable inline
  + per-env Set active / Duplicate / Delete header actions.
  (V5 cenário 10)
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
  refletindo edição manual de `connections.toml`. (V4 cenários 1-8)
- **`find_connection_uses_cmd` Tauri command** — vault-grep
  on-demand (`httui_core::connection_uses`) que walk `*.md` e
  retorna `{file, line}` de cada referência `db-<connection>` no
  vault. (V4 cenário 7)
- **Workbench shell + design system** — top bar com logo, breadcrumb
  (workspace › project › file), segmented env switcher, ⌘K search e
  branch button substituem a topbar legada. Sidebar nova reúne Files,
  Connections (status dot + latência ms + PROD chip) e Variables
  (lock icon + valor mascarado para entries `is_secret`). Status bar
  interativa expõe env menu, branch menu, contador `+N ~M -D` de
  mudanças git, latência, cursor (Ln/Col), encoding e versão.
  (V2 cenários 1-3)
- **Inline DocHeader (Notion-mode)** — título serif, abstract, tags
  (chips +/×) e checklist preflight (`[x] item`) editáveis dentro do
  CodeMirror; frontmatter YAML invisível e gerado automaticamente.
  Meta strip do header mostra autor (avatar Gravatar + nome),
  contagem de blocos e last-run inline. (V2 cenário 4.5)
- **Empty-state cards (Open / Clone / Create vault)** — primeiro
  contato com o app sem vault aberto. Three actionable cards
  replace the legacy "Em branco / Templates / Importar" surface,
  with inline error rendering per card and Mac-native directory
  picker. Open and Create rely on `scaffold_new_vault`; Clone
  shells out to `git` and respects the user's credential helper
  / ssh-agent. (V1 vertical 1, cenários 1-3)
- **`clone_vault_cmd` Tauri command** — `git clone <url>
  <parent>/<repo-name>` com leaf derivado da URL e parent
  configurável. Default parent: `~/Documents`. Pre-flight rejeita
  parent inexistente, parent que é arquivo, e leaf não-vazio.
  Backed by `httui_core::git::git_clone`. (V1 vertical 1, cenário 2)
- **`create_vault_cmd` Tauri command** — compõe mkdir + `git init`
  + `scaffold_new_vault` numa operação atômica do ponto de vista
  do user. Validações de input rejeitam path traversal (name vazio,
  com `/` ou `\\`, começando com `.`). Backed by
  `httui_core::vault_config::create::create_new_vault`.
  (V1 vertical 1, cenário 3)
- **First-run secrets modal** — quando o vault aberto referencia
  `{{keychain:...}}` ausentes do OS keychain local, abre um modal
  batch após `switchVault`. Cada row tem Save (preenche e remove
  do store) e Skip (esconde da sessão atual mas mantém pendente).
  Skip all / Done dismissam sem tocar o store. Refs ainda pendentes
  ficam visíveis via badge na statusbar (`LuTriangleAlert` +
  contador), clicável para reabrir o modal. (V1 vertical 1,
  cenário 4)
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
  the `*.local.toml` block. (Epic 10, ADR 0004)
- **File watcher** — the desktop app watches `connections.toml`,
  `envs/*.toml`, `.httui/workspace.toml` and `~/.config/httui/user.toml`
  via `notify`; external edits invalidate the in-process cache and
  emit a Tauri event. (Epic 11)
- **Vault migration tooling** — Tauri command `migrate_vault_to_v1`
  walks the legacy SQLite tables and writes the v1 file layout. Backs
  up `notes.db` first; idempotent on re-run; supports a dry-run
  preview. (Epic 12, see [`docs/MIGRATION.md`](docs/MIGRATION.md))
- **Secret backend abstraction** — `SecretBackend` trait with a
  `Keychain` default impl plus a parser for `{{keychain:…}}` markers in
  TOML. Slot for future `1Password` / `Stronghold` / `pass` impls.
  (Epic 13)
- **Vault open / scaffold / validate** — `open_vault`,
  `scaffold_vault`, `check_is_vault` Tauri commands; first-run flow
  for empty directories writes the v1 skeleton (`runbooks/`,
  `connections.toml`, `envs/`, `.httui/`, `.gitignore`). (Epic 17)
- **First-run missing-secrets scan** — `first_run_missing_secrets`
  Tauri command lists keychain markers referenced by the vault that
  have no value on this machine, so the UI can prompt for batch entry.
  (Epic 18)
- **Settings split foundation** — `user.toml` (per-machine prefs)
  vs. `.httui/workspace.toml` (vault defaults) split, with the seven
  legacy `app_config` UI keys promoted to the new schema. Schema
  bump shipped; UI restructure deferred to a frontend session. (Epic 19)
- **Git panel backend** — `httui_core::git` shells out to `git` for
  status, log, branch, fetch, pull, push and remote inspection;
  exposed through Tauri commands ready for the panel UI to consume.
  (Epic 20)
- **Codebase reorganization** — desktop app moved into
  `httui-desktop/`, marketing landing into `httui-web/`, chat sidecar
  into `httui-sidecar/`. Shared logic lives in `httui-core/`. The TUI,
  MCP server and chat sidecar all read the same vault on disk.
  (Epic 00)
- **Quality gates** — pre-push and CI gate every modified `.rs`/`.ts`/
  `.tsx` file at ≤600 production lines and ≥80% line coverage on the
  file as a whole; ESLint warnings for `complexity`,
  `max-lines-per-function`, `max-params`, `max-depth` baseline
  recorded. (Epic 04.5, Epic 04)
- **OSS readiness docs** — README, CONTRIBUTING, SECURITY,
  CODE_OF_CONDUCT, LICENSE plus `docs/ARCHITECTURE.md`, four ADRs
  and user-facing `docs/concepts.md` + `docs/blocks.md`.
  (Epics 01, 36, 37)

### Changed

- **Botão git da top bar** — agora abre/fecha o git side panel
  (antes abria direto o pane-tab). O pane-tab detalhado abre pelo
  "Details" / "View all" dentro do side panel. (V10.1 cenário 1)
- **Design system token vocabulary** — UI 100% alinhada ao
  vocabulário Chakra v3. Tokens custom (`bg.1/2/3/hi`,
  `fg.2/3`, `line`, `line.soft`, `accent.*`, `sel`) foram
  retirados em favor dos defaults Chakra (`bg.subtle/muted/
  emphasized/panel`, `fg.muted/subtle`, `border`, `brand.fg/
  contrast/subtle`). Recipes internos (Menu, Popover, Tooltip,
  Card, Badge) consomem os mesmos nomes — sem slot recipe
  override por componente. (V2 cenário 5)
- **File-tree contrast** — items inativos da árvore de arquivos
  passaram de `fg.subtle` para `fg.muted` para garantir
  legibilidade no tema dark Fuji. (V2 cenário 5)
- **MarkdownEditor split** — o componente monolítico
  (~573 linhas com `coverage:exclude`) foi quebrado em três
  sub-módulos coesos (`markdown-vim-motions`,
  `markdown-highlight-style`, `markdown-extensions`) com
  100% de cobertura, deixando o shell React em ~206 linhas.
  Comportamento user-visible inalterado. (V2 cenário 6)
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
  on read. (Epic 24)

### Removed

- **Aba Audit do git panel** — sem os filtros action-type
  (adiados pra v1.x) era idêntica à aba Log; removida do v1, volta
  com os filtros. (V10 follow-up)
- **Pre-flight `keychain` kind** — retirado do typed set em V6.
  macOS keychain enumeration é restritivo e os call sites que se
  beneficiariam não estão construídos. YAML legado com
  `keychain: <key>` cai pro fallback `Unknown` do parser (não
  crasha — só não aparece como pill). (V6 cenário 9 — `3c3be69`)
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
  scope for v1 (`docs-llm/v1/out-of-scope.md`); marketing landing
  copy trimmed to match.

### Fixed

- **Erro de push ilegível** — rejeições do push (branch protegida
  / GH013 / non-fast-forward / auth) eram despejadas como o stderr
  cru do git, espremido e ininteligível. Agora vêm com um resumo
  legível em destaque + o detalhe limpo (sem o ruído `remote:`)
  num bloco rolável; o botão volta a "Retry sync". (V10.1
  cenário 3, follow-up de UAT)
- **Conflitos de merge invisíveis no git panel** — `git status`
  não interpretava as linhas `u` (unmerged) do `porcelain=v2`, então
  um vault em conflito aparecia como "Working tree clean" e o banner
  de resolução nunca surgia. (V10 cenário 6)
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

[Unreleased]: https://github.com/httuicom/httui/commits/main
