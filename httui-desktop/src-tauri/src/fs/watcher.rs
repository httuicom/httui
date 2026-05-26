// coverage:exclude file — notify event loop + Tauri emitters require a real filesystem and AppHandle.

use httui_core::vault_config::watch_paths::{classify, env_name_from_path, WatchCategory};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum FileEvent {
    Created { path: String },
    Modified { path: String },
    Removed { path: String },
}

/// Emitted when an externally modified .md file is read back from disk.
#[derive(Debug, Clone, Serialize)]
pub struct FileReloaded {
    pub path: String,
    pub markdown: String,
}

/// Emitted when a watched config TOML file changes. The frontend re-fetches the relevant store on receipt.
#[derive(Debug, Clone, Serialize)]
pub struct ConfigChanged {
    pub category: WatchCategory,
    /// Vault-relative path that changed (forward slashes).
    pub path: String,
    /// For `category == "env"`, the env name (filename stem with the
    /// optional `.local` suffix stripped). `None` for connections /
    /// workspace.
    pub env: Option<String>,
}

pub struct VaultWatcher {
    _watcher: RecommendedWatcher,
}

pub fn watch_vault(
    vault_path: &str,
    app_handle: AppHandle,
    ignore_paths: Arc<Mutex<Vec<String>>>,
) -> Result<VaultWatcher, String> {
    let vault = vault_path.to_string();
    let (tx, rx) = mpsc::channel();

    let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(event) = res {
            let _ = tx.send(event);
        }
    })
    .map_err(|e| e.to_string())?;

    watcher
        .watch(Path::new(vault_path), RecursiveMode::Recursive)
        .map_err(|e| e.to_string())?;

    let handle = app_handle.clone();
    let vault_for_thread = vault.clone();
    std::thread::spawn(move || {
        let md_debounce = Duration::from_millis(500);
        // TOML: 250ms trailing debounce so git-pull/editor-save bursts coalesce.
        let toml_debounce = Duration::from_millis(250);
        let mut last_emit_per_file: HashMap<String, Instant> = HashMap::new();

        for event in rx {
            #[derive(Clone)]
            enum Kind {
                Md,
                Config(WatchCategory),
            }
            let entries: Vec<(String, Kind)> = event
                .paths
                .iter()
                .filter_map(|p| {
                    let s = p.to_string_lossy().to_string();
                    let rel = p
                        .strip_prefix(&vault_for_thread)
                        .unwrap_or(p)
                        .to_string_lossy()
                        .trim_start_matches('/')
                        .to_string();

                    if s.ends_with(".md") && !s.contains("/.") && !s.contains("\\.") {
                        return Some((rel, Kind::Md));
                    }

                    if s.ends_with(".toml") {
                        if let Some(cat) = classify(&rel) {
                            return Some((rel, Kind::Config(cat)));
                        }
                    }
                    None
                })
                .collect();

            if entries.is_empty() {
                continue;
            }

            {
                let ignored = ignore_paths.lock().unwrap();
                if entries.iter().any(|(p, _)| ignored.contains(p)) {
                    continue;
                }
            }

            for (path, kind) in entries {
                let debounce = match kind {
                    Kind::Md => md_debounce,
                    Kind::Config(_) => toml_debounce,
                };
                if let Some(last) = last_emit_per_file.get(&path) {
                    if last.elapsed() < debounce {
                        continue;
                    }
                }
                last_emit_per_file.insert(path.clone(), Instant::now());

                match kind {
                    Kind::Md => emit_md_event(&handle, &vault_for_thread, &event.kind, &path),
                    Kind::Config(cat) => emit_config_changed(&handle, cat, &path),
                }
            }
        }
    });

    Ok(VaultWatcher { _watcher: watcher })
}

fn emit_md_event(handle: &AppHandle, vault: &str, kind: &EventKind, path: &str) {
    match kind {
        EventKind::Modify(_) => match crate::fs::read_note(vault, path) {
            Ok(markdown) => {
                let _ = handle.emit(
                    "file-reloaded",
                    FileReloaded {
                        path: path.to_string(),
                        markdown,
                    },
                );
            }
            Err(_) => {
                // File may be mid-write; fall back to a plain event.
                let _ = handle.emit(
                    "fs-event",
                    FileEvent::Modified {
                        path: path.to_string(),
                    },
                );
            }
        },
        EventKind::Create(_) => {
            let _ = handle.emit(
                "fs-event",
                FileEvent::Created {
                    path: path.to_string(),
                },
            );
        }
        EventKind::Remove(_) => {
            let _ = handle.emit(
                "fs-event",
                FileEvent::Removed {
                    path: path.to_string(),
                },
            );
        }
        _ => {}
    }
}

fn emit_config_changed(handle: &AppHandle, category: WatchCategory, path: &str) {
    let env = matches!(category, WatchCategory::Env)
        .then(|| env_name_from_path(path).map(|s| s.to_string()))
        .flatten();
    let _ = handle.emit(
        "config-changed",
        ConfigChanged {
            category,
            path: path.to_string(),
            env,
        },
    );
}
