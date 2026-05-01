// coverage:exclude file — Tauri command orchestrator + state wiring +
// setup hooks. No extractable logic remains after Epic 20a Story 05;
// the substantive code lives in `httui-core` and per-domain
// `commands/*.rs` modules, each tested independently. The
// size:exclude opt-out came off in commit XXX (Story 05 closeout) —
// main.rs is now 547 prod lines, under the 600 gate.
//
// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use sqlx::sqlite::SqlitePool;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

use httui_notes::chat::commands::*;
use httui_notes::db::connections::{PoolManager, StatusEmitter};

// --- Tauri StatusEmitter implementation ---

#[derive(Clone, serde::Serialize)]
struct ConnectionStatusEvent {
    connection_id: String,
    name: String,
    status: String,
}

struct TauriStatusEmitter {
    app_handle: AppHandle,
}

impl StatusEmitter for TauriStatusEmitter {
    fn emit_connection_status(&self, connection_id: &str, name: &str, status: &str) {
        let _ = self.app_handle.emit(
            "connection-status",
            ConnectionStatusEvent {
                connection_id: connection_id.to_string(),
                name: name.to_string(),
                status: status.to_string(),
            },
        );
    }
}

// Block-related commands (execute_block, result cache, history,
// settings, examples, hash computation) and the SharedDbExecutor /
// SharedHttpExecutor registry wrappers moved to `commands::blocks`
// (Epic 20a Story 05).

// Schema commands moved to `commands::schema` (Epic 20a Story 05).
// Config commands moved to `commands::settings` (Epic 20a Story 05).

// Vault file commands moved to `commands::files` (Epic 20a Story 05).

/// Re-read a file from disk and emit `file-reloaded` so the editor
/// replaces its in-memory copy. Used after MCP writes to defeat the
/// auto-save suppression window.
#[tauri::command]
fn force_reload_file(
    vault_path: String,
    file_path: String,
    app_handle: AppHandle,
) -> Result<(), String> {
    let markdown = httui_notes::fs::read_note(&vault_path, &file_path)?;
    app_handle
        .emit(
            "file-reloaded",
            httui_notes::fs::watcher::FileReloaded {
                path: file_path,
                markdown,
            },
        )
        .map_err(|e| e.to_string())
}

/// Start the `notify`-backed file watcher for `vault_path`. Subsequent
/// changes outside our own writes surface as `file-changed` events.
#[tauri::command]
fn start_watching(
    vault_path: String,
    app_handle: tauri::AppHandle,
    ignore_paths: tauri::State<'_, Arc<Mutex<Vec<String>>>>,
    watcher_state: tauri::State<'_, Mutex<Option<httui_notes::fs::watcher::VaultWatcher>>>,
) -> Result<(), String> {
    let watcher = httui_notes::fs::watcher::watch_vault(
        &vault_path,
        app_handle,
        ignore_paths.inner().clone(),
    )?;
    let mut state = watcher_state.lock().unwrap();
    *state = Some(watcher);
    Ok(())
}

/// Quick-open fuzzy file-name search across the vault. Backed by a
/// subsequence-scoring matcher in `httui-core::search`.
#[tauri::command]
fn search_files(
    vault_path: String,
    query: String,
) -> Result<Vec<httui_notes::search::SearchResult>, String> {
    httui_notes::search::search_files(&vault_path, &query)
}

/// Vault grep for `{{KEY}}` and `{{KEY.…}}` references. Powers the
/// "Used in blocks" detail-panel section in the Variables page
/// (Epic 43 Story 04). Pure file scan — no FTS5 dependency.
#[tauri::command]
fn grep_var_uses(
    vault_path: String,
    key: String,
) -> Result<Vec<httui_notes::var_uses::VarUseEntry>, String> {
    httui_notes::var_uses::grep_var_uses(&vault_path, &key)
}

/// Rebuild the SQLite FTS5 index for the vault. Called on first run
/// and when switching vaults.
#[tauri::command]
async fn rebuild_search_index(
    vault_path: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    httui_notes::search::rebuild_search_index(&pool, &vault_path).await
}

/// Full-text search (Cmd+Shift+F) returning highlighted snippets from
/// the FTS5 index.
#[tauri::command]
async fn search_content(
    query: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<httui_notes::search::ContentSearchResult>, String> {
    httui_notes::search::search_content(&pool, &query).await
}

/// Refresh a single FTS row (called on save) so search picks up edits
/// without rebuilding the whole index.
#[tauri::command]
async fn update_search_entry(
    file_path: String,
    content: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    httui_notes::search::update_search_entry(&pool, &file_path, &content).await
}

/// Drop the active file watcher (e.g. on vault switch).
#[tauri::command]
fn stop_watching(
    watcher_state: tauri::State<'_, Mutex<Option<httui_notes::fs::watcher::VaultWatcher>>>,
) -> Result<(), String> {
    let mut state = watcher_state.lock().unwrap();
    *state = None;
    Ok(())
}

// Connection + environment commands moved to `commands::connections`
// and `commands::environments` (Epic 19 Story 02 Phase 2 + 3 —
// file-backed cutover; audit-015). Wire-compat is preserved
// (Connection.id == name; Environment.id == name;
// EnvVariable.id == "<env>::<key>").

// --- Internal DB query (audit/settings) ---

/// Run a SELECT against the app's own SQLite (audit/settings panel).
/// Multi-statements and writes are rejected; pagination via
/// `(offset, fetch_size)`.
#[tauri::command]
async fn query_internal_db(
    pool: tauri::State<'_, SqlitePool>,
    query: String,
    offset: u32,
    fetch_size: u32,
) -> Result<httui_notes::db::InternalQueryResult, String> {
    httui_notes::db::query_internal_db(&pool, &query, offset, fetch_size).await
}

// --- Session restore (single IPC call for startup) ---

#[derive(serde::Serialize)]
struct SessionTabContent {
    file_path: String,
    vault_path: String,
    content: Option<String>,
}

#[derive(serde::Serialize)]
struct SessionState {
    vaults: Vec<String>,
    active_vault: Option<String>,
    vim_enabled: bool,
    sidebar_open: bool,
    pane_layout: Option<String>,
    active_pane_id: Option<String>,
    active_file: Option<String>,
    scroll_positions: Option<String>,
    file_tree: Vec<httui_notes::fs::FileEntry>,
    tab_contents: Vec<SessionTabContent>,
}

// Extracts tab file paths from pane layout JSON
fn extract_tabs_from_layout(value: &serde_json::Value) -> Vec<(String, String)> {
    let mut tabs = Vec::new();
    if let Some(typ) = value.get("type").and_then(|t| t.as_str()) {
        if typ == "leaf" {
            if let Some(tab_arr) = value.get("tabs").and_then(|t| t.as_array()) {
                for tab in tab_arr {
                    if let (Some(fp), Some(vp)) = (
                        tab.get("filePath").and_then(|v| v.as_str()),
                        tab.get("vaultPath").and_then(|v| v.as_str()),
                    ) {
                        tabs.push((fp.to_string(), vp.to_string()));
                    }
                }
            }
        } else if typ == "split" {
            if let Some(children) = value.get("children").and_then(|c| c.as_array()) {
                for child in children {
                    tabs.extend(extract_tabs_from_layout(child));
                }
            }
        }
    }
    tabs
}

/// Single-shot startup IPC: load config keys, parse the pane layout,
/// list the workspace, and read every open tab's content concurrently.
/// Replaces ~10 chatty calls so the editor renders without flicker.
#[tauri::command]
async fn restore_session(pool: tauri::State<'_, SqlitePool>) -> Result<SessionState, String> {
    // Batch all config reads concurrently
    let (
        vaults_raw,
        vim_raw,
        sidebar_raw,
        active_vault,
        pane_layout,
        active_pane_id,
        active_file,
        scroll_positions,
    ) = tokio::join!(
        httui_notes::config::get_config(&pool, "vaults"),
        httui_notes::config::get_config(&pool, "vim_enabled"),
        httui_notes::config::get_config(&pool, "sidebar_open"),
        httui_notes::config::get_config(&pool, "active_vault"),
        httui_notes::config::get_config(&pool, "pane_layout"),
        httui_notes::config::get_config(&pool, "active_pane_id"),
        httui_notes::config::get_config(&pool, "active_file"),
        httui_notes::config::get_config(&pool, "scroll_positions"),
    );

    let vaults: Vec<String> = vaults_raw
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();

    let vim_enabled = vim_raw.ok().flatten().as_deref() == Some("true");
    let sidebar_open = sidebar_raw.ok().flatten().as_deref() != Some("false");
    let active_vault = active_vault.ok().flatten();
    let pane_layout = pane_layout.ok().flatten();
    let active_pane_id = active_pane_id.ok().flatten();
    let active_file = active_file.ok().flatten();
    let scroll_positions = scroll_positions.ok().flatten();

    // Extract tab file paths from saved layout (done in Rust, no extra roundtrip)
    let tab_files: Vec<(String, String)> = if let Some(ref layout_json) = pane_layout {
        serde_json::from_str::<serde_json::Value>(layout_json)
            .map(|v| extract_tabs_from_layout(&v))
            .unwrap_or_default()
    } else if let (Some(ref file), Some(ref vault)) = (&active_file, &active_vault) {
        vec![(file.clone(), vault.clone())]
    } else {
        vec![]
    };

    // Run list_workspace + read all tab files in parallel using blocking tasks
    let active_vault_clone = active_vault.clone();
    let tree_handle = tokio::task::spawn_blocking(move || {
        if let Some(ref vault) = active_vault_clone {
            httui_notes::fs::list_workspace(vault).unwrap_or_default()
        } else {
            vec![]
        }
    });

    let mut file_handles = Vec::new();
    for (file_path, vault_path) in tab_files {
        let fp = file_path.clone();
        let vp = vault_path.clone();
        file_handles.push(tokio::task::spawn_blocking(move || {
            let content = httui_notes::fs::read_note(&vp, &fp).ok();
            SessionTabContent {
                file_path: fp,
                vault_path: vp,
                content,
            }
        }));
    }

    let file_tree = tree_handle.await.unwrap_or_default();
    let mut tab_contents = Vec::new();
    for handle in file_handles {
        if let Ok(tab) = handle.await {
            tab_contents.push(tab);
        }
    }
    Ok(SessionState {
        vaults,
        active_vault,
        vim_enabled,
        sidebar_open,
        pane_layout,
        active_pane_id,
        active_file,
        scroll_positions,
        file_tree,
        tab_contents,
    })
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_sql::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let app_data_dir =
                httui_core::paths::default_data_dir().expect("failed to resolve data dir");

            match httui_core::paths::migrate_legacy_data(&app_data_dir) {
                Ok(httui_core::paths::MigrationOutcome::Migrated { from }) => {
                    eprintln!(
                        "[migration] copied legacy data from {} to {}",
                        from.display(),
                        app_data_dir.display()
                    );
                }
                Ok(_) => {}
                Err(e) => eprintln!("[migration] failed: {e}"),
            }

            let pool = tauri::async_runtime::block_on(async {
                httui_notes::db::init_db(&app_data_dir)
                    .await
                    .expect("failed to initialize database")
            });

            app.manage(pool.clone());

            // Per-vault store registry resolves ConnectionsStore +
            // EnvironmentsStore for the active vault (Epic 19 Story 02
            // Phase 1 — commit 037f470).
            let store_registry =
                httui_notes::commands::vault_stores::VaultStoreRegistry::new();

            // Connection lookup wrapper threads the registry into the
            // pool manager so pool builds resolve connections from the
            // file-backed store, not legacy SQLite (Phase 3).
            let conn_lookup = httui_notes::commands::vault_stores::VaultRegistryLookup::new(
                pool.clone(),
                store_registry.clone(),
            );

            // Connection pool manager
            let emitter = Arc::new(TauriStatusEmitter {
                app_handle: app.handle().clone(),
            });
            let conn_manager = Arc::new(PoolManager::new_with_emitter(
                conn_lookup,
                pool.clone(),
                emitter,
            ));
            app.manage(conn_manager.clone());

            // TTL cleanup + query log retention task
            let cm = conn_manager.clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(60));
                let mut log_cleanup_counter: u32 = 0;
                loop {
                    interval.tick().await;
                    cm.cleanup_expired().await;
                    // Clean query_log every ~30 min (30 ticks of 60s)
                    log_cleanup_counter += 1;
                    if log_cleanup_counter >= 30 {
                        log_cleanup_counter = 0;
                        cm.cleanup_query_log().await;
                    }
                }
            });

            // Executor registry. DbExecutor is held as Arc<…> so the
            // cancel-aware streamed command (see src/executions.rs) can
            // share a single instance with the legacy `execute_block`.
            let db_executor = Arc::new(httui_notes::executor::db::DbExecutor::new(conn_manager));
            app.manage(db_executor.clone());
            app.manage(httui_notes::executions::ExecutionRegistry::new());

            // HTTP executor is held as Arc<…> so the cancel-aware streamed
            // command can share a single instance with the legacy `execute_block`.
            let http_executor = Arc::new(httui_notes::executor::http::HttpExecutor::new());
            app.manage(http_executor.clone());

            let mut executor_registry = httui_notes::executor::ExecutorRegistry::new();
            executor_registry.register(Box::new(httui_notes::commands::blocks::SharedHttpExecutor(
                http_executor,
            )));
            executor_registry.register(Box::new(httui_notes::commands::blocks::SharedDbExecutor(
                db_executor,
            )));
            app.manage(executor_registry);

            // Chat sidecar (lazy — spawned on first use, not at startup)
            app.manage(std::sync::Arc::new(tokio::sync::Mutex::new(
                None::<httui_notes::chat::sidecar::SidecarManager>,
            )));

            // Permission broker
            let pool_for_broker: SqlitePool = app.state::<SqlitePool>().inner().clone();
            app.manage(Arc::new(
                httui_notes::chat::permissions::PermissionBroker::new(pool_for_broker),
            ));

            app.manage(Arc::new(Mutex::new(Vec::<String>::new()))); // ignore_paths
            app.manage(Mutex::new(None::<httui_notes::fs::watcher::VaultWatcher>));

            // Per-vault file-backed store registry (Epic 19 cutover —
            // audit-015). Resolves ConnectionsStore + EnvironmentsStore
            // for the active vault on demand, caches per vault path.
            app.manage(store_registry);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            httui_notes::commands::blocks::execute_block,
            httui_notes::executions::execute_db_streamed,
            httui_notes::executions::execute_http_streamed,
            httui_notes::executions::cancel_block,
            httui_notes::commands::blocks::list_block_history,
            httui_notes::commands::blocks::list_block_history_for_file,
            httui_notes::commands::blocks::insert_block_history,
            httui_notes::commands::blocks::purge_block_history,
            httui_notes::commands::blocks::get_block_settings,
            httui_notes::commands::blocks::upsert_block_settings,
            httui_notes::commands::blocks::purge_block_settings,
            httui_notes::commands::blocks::save_block_example,
            httui_notes::commands::blocks::list_block_examples,
            httui_notes::commands::blocks::delete_block_example,
            httui_notes::commands::blocks::purge_block_examples,
            httui_notes::commands::blocks::get_block_result,
            httui_notes::commands::blocks::save_block_result,
            httui_notes::commands::blocks::compute_block_hash,
            httui_notes::commands::settings::get_config,
            httui_notes::commands::settings::set_config,
            // Epic 09 foundation — file-backed workspace + user config.
            // Frontend cutover lands in epic 19 (settings split).
            httui_notes::vault_config_commands::get_workspace_config,
            httui_notes::vault_config_commands::set_workspace_config,
            httui_notes::vault_config_commands::get_file_settings,
            httui_notes::vault_config_commands::set_file_auto_capture,
            httui_notes::vault_config_commands::set_file_docheader_compact,
            httui_notes::vault_config_commands::get_user_config,
            httui_notes::vault_config_commands::set_user_config,
            // Epic 10 — local override gitignore scaffolding.
            httui_notes::vault_config_commands::ensure_vault_gitignore,
            // Epic 12 — vault migration script.
            httui_notes::vault_config_commands::migrate_vault_to_v1,
            // Epic 41 Story 07 — empty-state migration banner detection.
            httui_notes::vault_config_commands::detect_vault_migration,
            // Epic 17 — vault scaffold + validate.
            httui_notes::vault_config_commands::check_is_vault,
            httui_notes::vault_config_commands::scaffold_vault,
            // Epic 18 — first-run missing-secrets scan.
            httui_notes::vault_config_commands::list_missing_secrets,
            // Epic 20 — git panel.
            httui_notes::git_commands::git_status_cmd,
            httui_notes::git_commands::git_log_cmd,
            httui_notes::git_commands::git_diff_cmd,
            httui_notes::git_commands::git_branch_list_cmd,
            httui_notes::git_commands::git_remote_list_cmd,
            httui_notes::git_commands::git_first_commit_author_cmd,
            httui_notes::git_commands::git_checkout_cmd,
            httui_notes::git_commands::git_checkout_conflict_path_cmd,
            httui_notes::git_commands::git_checkout_b_cmd,
            httui_notes::git_commands::stage_path_cmd,
            httui_notes::git_commands::unstage_path_cmd,
            httui_notes::git_commands::git_commit_cmd,
            httui_notes::git_commands::git_fetch_cmd,
            httui_notes::git_commands::git_pull_cmd,
            httui_notes::git_commands::git_push_cmd,
            // Epic 52 Story 04 — vault-wide tag index.
            httui_notes::tag_commands::scan_vault_tags_cmd,
            // Epic 47 Story 01 — run-body filesystem cache.
            httui_notes::run_body_commands::write_run_body_cmd,
            httui_notes::run_body_commands::read_run_body_cmd,
            httui_notes::run_body_commands::list_run_bodies_cmd,
            httui_notes::run_body_commands::trim_run_bodies_cmd,
            httui_notes::run_body_commands::rename_alias_runs_cmd,
            // Epic 41 Story 04 — template registry.
            httui_notes::templates_commands::list_templates_cmd,
            // Epic 46 Story 03 — captures persistence.
            httui_notes::captures_commands::read_captures_cache_cmd,
            httui_notes::captures_commands::write_captures_cache_cmd,
            httui_notes::captures_commands::delete_captures_cache_cmd,
            restore_session,
            httui_notes::commands::files::list_workspace,
            httui_notes::commands::files::read_note,
            httui_notes::commands::files::write_note,
            httui_notes::commands::files::create_note,
            httui_notes::commands::files::delete_note,
            httui_notes::commands::files::rename_note,
            httui_notes::commands::files::create_folder,
            httui_notes::commands::files::get_file_mtime,
            start_watching,
            stop_watching,
            search_files,
            grep_var_uses,
            rebuild_search_index,
            search_content,
            update_search_entry,
            httui_notes::commands::connections::list_connections,
            httui_notes::commands::connections::create_connection,
            httui_notes::commands::connections::update_connection,
            httui_notes::commands::connections::delete_connection,
            httui_notes::commands::connections::test_connection,
            httui_notes::commands::schema::introspect_schema,
            httui_notes::commands::schema::get_cached_schema,
            httui_notes::commands::environments::list_environments,
            httui_notes::commands::environments::create_environment,
            httui_notes::commands::environments::delete_environment,
            httui_notes::commands::environments::duplicate_environment,
            httui_notes::commands::environments::set_active_environment,
            httui_notes::commands::environments::list_env_variables,
            httui_notes::commands::environments::set_env_variable,
            httui_notes::commands::environments::delete_env_variable,
            // Chat
            create_chat_session,
            list_chat_sessions,
            get_chat_session,
            archive_chat_session,
            list_chat_messages,
            send_chat_message,
            abort_chat,
            respond_chat_permission,
            save_attachment_tmp,
            clear_session_claude_id,
            update_chat_session_cwd,
            delete_messages_after,
            list_tool_permissions,
            delete_tool_permission,
            get_usage_stats,
            force_reload_file,
            query_internal_db,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let app = window.app_handle().clone();
                let sidecar_state = app.state::<std::sync::Arc<
                    tokio::sync::Mutex<Option<httui_notes::chat::sidecar::SidecarManager>>,
                >>();
                let sidecar = sidecar_state.inner().clone();
                tauri::async_runtime::spawn(async move {
                    let guard = sidecar.lock().await;
                    if let Some(mgr) = guard.as_ref() {
                        mgr.shutdown().await;
                    }
                });
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
