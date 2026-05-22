use serde::Deserialize;
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use uuid::Uuid;

pub type SidecarState = Arc<Mutex<Option<SidecarManager>>>;

use httui_core::db::chat::{self, Message, Session};

use super::permissions::{PermissionBroker, PermissionVerdict};
use super::protocol::*;
use super::sidecar::SidecarManager;

pub type PermissionBrokerState = Arc<PermissionBroker>;

#[derive(Debug, Deserialize)]
pub struct AttachmentInput {
    pub media_type: String,
    pub path: String,
}

/// Create a new chat session (optional `cwd` overrides the active vault
/// path used as Claude's working directory).
#[tauri::command]
pub async fn create_chat_session(
    pool: tauri::State<'_, SqlitePool>,
    cwd: Option<String>,
) -> Result<Session, String> {
    chat::create_session(&pool, cwd).await
}

/// List non-archived chat sessions, most recent first.
#[tauri::command]
pub async fn list_chat_sessions(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<Session>, String> {
    chat::list_sessions(&pool).await
}

/// Fetch a single session row including its `claude_session_id` for
/// resume.
#[tauri::command]
pub async fn get_chat_session(
    pool: tauri::State<'_, SqlitePool>,
    session_id: i64,
) -> Result<Session, String> {
    chat::get_session(&pool, session_id).await
}

/// Mark a session as archived. Sessions are kept in SQLite (never
/// hard-deleted) so message history survives.
#[tauri::command]
pub async fn archive_chat_session(
    pool: tauri::State<'_, SqlitePool>,
    session_id: i64,
) -> Result<(), String> {
    chat::archive_session(&pool, session_id).await
}

/// List the messages of a session, oldest first, ready for the
/// transcript renderer.
#[tauri::command]
pub async fn list_chat_messages(
    pool: tauri::State<'_, SqlitePool>,
    session_id: i64,
) -> Result<Vec<Message>, String> {
    chat::list_messages(&pool, session_id).await
}

/// Persist a user message, normalize attached images (resize > 2048px,
/// re-encode JPEG Q85), resolve `[[wikilinks]]`, and ship the payload to
/// the sidecar. Streams events back via `chat:delta`/`chat:done`.
#[tauri::command]
pub async fn send_chat_message(
    app: AppHandle,
    pool: tauri::State<'_, SqlitePool>,
    sidecar: tauri::State<'_, SidecarState>,
    broker: tauri::State<'_, PermissionBrokerState>,
    session_id: i64,
    text: String,
    attachments: Vec<AttachmentInput>,
) -> Result<String, String> {
    let mut db_blocks: Vec<serde_json::Value> =
        vec![serde_json::json!({"type": "text", "text": text})];
    let mut sidecar_blocks: Vec<serde_json::Value> =
        vec![serde_json::json!({"type": "text", "text": text})];

    for att in &attachments {
        let bytes = tokio::fs::read(&att.path)
            .await
            .map_err(|e| format!("Failed to read attachment {}: {e}", att.path))?;

        let (normalized, norm_media_type) = normalize_image(&bytes, &att.media_type)
            .unwrap_or_else(|_| (bytes.clone(), att.media_type.clone()));
        let b64 = base64_encode(&normalized);

        db_blocks.push(serde_json::json!({
            "type": "image",
            "path": att.path,
            "media_type": att.media_type,
        }));

        sidecar_blocks.push(serde_json::json!({
            "type": "image",
            "source": {
                "type": "base64",
                "media_type": norm_media_type,
                "data": b64,
            }
        }));
    }

    let content_json = serde_json::to_string(&db_blocks)
        .map_err(|e| format!("Failed to serialize content: {e}"))?;

    chat::insert_message(&pool, session_id, "user", &content_json, None, None, false).await?;

    let session_for_title = chat::get_session(&pool, session_id).await?;
    if session_for_title.title == "Nova conversa" {
        let title: String = text.chars().take(50).collect();
        let title = title
            .split('\n')
            .next()
            .unwrap_or(&title)
            .trim()
            .to_string();
        if !title.is_empty() {
            let _ = chat::update_session_title(&pool, session_id, &title).await;
            let _ = app.emit("chat:session-updated", session_id);
        }
    }

    let session = chat::get_session(&pool, session_id).await?;

    // Use session cwd, or fall back to active vault path
    let effective_cwd = if session.cwd.is_some() {
        session.cwd.clone()
    } else {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM app_config WHERE key = 'active_vault'")
                .fetch_optional(pool.inner())
                .await
                .ok()
                .flatten();
        row.map(|r| r.0)
    };

    if let Some(ref cwd) = effective_cwd {
        let wikilink_re =
            regex::Regex::new(r"\[\[([^\]]+)\]\]").expect("static wikilink regex compiles");
        for cap in wikilink_re.captures_iter(&text) {
            let target = &cap[1];
            if let Ok(content) = resolve_wikilink(cwd, target) {
                sidecar_blocks.push(serde_json::json!({
                    "type": "text",
                    "text": format!("\n---\n[Referenced note: {target}]\n{content}\n---\n"),
                }));
            }
        }
    }

    let effective_cwd_for_broker = effective_cwd.clone();
    let request_id = Uuid::new_v4().to_string();
    {
        let mut guard = sidecar.lock().await;
        if guard.is_none() {
            let mgr = SidecarManager::spawn(&app)
                .await
                .map_err(|e| format!("Failed to spawn sidecar: {e}"))?;
            *guard = Some(mgr);
        }
    }
    let mut rx = {
        let sidecar_guard = sidecar.lock().await;
        let mgr = sidecar_guard
            .as_ref()
            .expect("sidecar guard set to Some above");
        let rx = mgr.register_request(&request_id).await;

        mgr.send(OutgoingMessage::Chat {
            request_id: request_id.clone(),
            claude_session_id: session.claude_session_id,
            cwd: effective_cwd,
            allowed_tools: vec![
                "Read".to_string(),
                "Glob".to_string(),
                "Grep".to_string(),
                "Bash".to_string(),
                "Edit".to_string(),
                "Write".to_string(),
            ],
            content: sidecar_blocks,
        })
        .await?;
        rx
    }; // sidecar_guard dropped here

    let pool_clone = pool.inner().clone();
    let request_id_clone = request_id.clone();
    let sidecar_clone = sidecar.inner().clone();
    let broker_clone = broker.inner().clone();
    let effective_cwd_clone = effective_cwd_for_broker;

    tauri::async_runtime::spawn(async move {
        let mut segments: Vec<serde_json::Value> = Vec::new();
        let mut current_text = String::new();
        let mut current_tool_ids: Vec<String> = Vec::new();
        let mut assistant_msg_id: Option<i64> = None;

        while let Some(msg) = rx.recv().await {
            match msg {
                IncomingMessage::Session {
                    claude_session_id, ..
                } => {
                    let _ =
                        chat::update_session_claude_id(&pool_clone, session_id, &claude_session_id)
                            .await;
                }
                IncomingMessage::TextDelta { text, .. } => {
                    if !current_tool_ids.is_empty() {
                        segments.push(serde_json::json!({"type": "tool_group", "tool_use_ids": current_tool_ids}));
                        current_tool_ids = Vec::new();
                    }
                    current_text.push_str(&text);
                    let _ = app.emit("chat:delta", ChatDeltaEvent { session_id, text });
                }
                IncomingMessage::ToolUse {
                    tool_use_id,
                    name,
                    input,
                    ..
                } => {
                    if !current_text.is_empty() {
                        segments.push(serde_json::json!({"type": "text", "text": current_text}));
                        current_text = String::new();
                    }
                    current_tool_ids.push(tool_use_id.clone());

                    if assistant_msg_id.is_none() {
                        let content = serde_json::json!(&segments);
                        if let Ok(msg) = chat::insert_message(
                            &pool_clone,
                            session_id,
                            "assistant",
                            &content.to_string(),
                            None,
                            None,
                            true,
                        )
                        .await
                        {
                            assistant_msg_id = Some(msg.id);
                        }
                    }
                    if let Some(msg_id) = assistant_msg_id {
                        let input_str = serde_json::to_string(&input).unwrap_or_default();
                        let _ = chat::insert_tool_call(
                            &pool_clone,
                            msg_id,
                            &tool_use_id,
                            &name,
                            &input_str,
                        )
                        .await;
                    }
                    let _ = app.emit(
                        "chat:tool_use",
                        ChatToolUseEvent {
                            session_id,
                            tool_use_id,
                            name,
                            input,
                        },
                    );
                }
                IncomingMessage::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                    ..
                } => {
                    let result_str = serde_json::to_string(&content).unwrap_or_default();
                    let _ = chat::update_tool_call_result(
                        &pool_clone,
                        &tool_use_id,
                        &result_str,
                        is_error,
                    )
                    .await;
                    let _ = app.emit(
                        "chat:tool_result",
                        ChatToolResultEvent {
                            session_id,
                            tool_use_id,
                            content,
                            is_error,
                        },
                    );
                }
                IncomingMessage::PermissionRequest {
                    permission_id,
                    tool_name,
                    tool_input,
                    ..
                } => {
                    let verdict = broker_clone
                        .check(
                            &tool_name,
                            &tool_input,
                            session_id,
                            effective_cwd_clone.as_deref(),
                        )
                        .await;

                    match verdict {
                        PermissionVerdict::Allow => {
                            if let Some(mgr) = sidecar_clone.lock().await.as_ref() {
                                let _ = mgr
                                    .send(OutgoingMessage::PermissionResponse {
                                        permission_id,
                                        decision: PermissionDecision {
                                            behavior: PermissionBehavior::Allow,
                                            message: None,
                                        },
                                    })
                                    .await;
                            }
                        }
                        PermissionVerdict::Deny(reason) => {
                            if let Some(mgr) = sidecar_clone.lock().await.as_ref() {
                                let _ = mgr
                                    .send(OutgoingMessage::PermissionResponse {
                                        permission_id,
                                        decision: PermissionDecision {
                                            behavior: PermissionBehavior::Deny,
                                            message: Some(reason),
                                        },
                                    })
                                    .await;
                            }
                        }
                        PermissionVerdict::AskUser => {
                            let _ = app.emit(
                                "chat:permission_request",
                                ChatPermissionRequestEvent {
                                    session_id,
                                    permission_id,
                                    tool_name,
                                    tool_input,
                                },
                            );
                        }
                    }
                }
                IncomingMessage::Done {
                    usage, stop_reason, ..
                } => {
                    if !current_tool_ids.is_empty() {
                        segments.push(serde_json::json!({"type": "tool_group", "tool_use_ids": current_tool_ids}));
                    }
                    if !current_text.is_empty() {
                        segments.push(serde_json::json!({"type": "text", "text": current_text}));
                    }
                    let content = serde_json::json!(&segments);
                    let tokens_in = usage.as_ref().map(|u| u.input_tokens as i64);
                    let tokens_out = usage.as_ref().map(|u| u.output_tokens as i64);
                    let cache_read = usage.as_ref().map(|u| u.cache_read_tokens as i64);

                    if let Some(msg_id) = assistant_msg_id {
                        let _ = sqlx::query(
                            "UPDATE messages SET content_json = ?, tokens_in = ?, tokens_out = ?, cache_read_tokens = ?, is_partial = 0 WHERE id = ?"
                        )
                        .bind(content.to_string())
                        .bind(tokens_in)
                        .bind(tokens_out)
                        .bind(cache_read)
                        .bind(msg_id)
                        .execute(&pool_clone)
                        .await;
                    } else {
                        let _ = chat::insert_message(
                            &pool_clone,
                            session_id,
                            "assistant",
                            &content.to_string(),
                            tokens_in,
                            tokens_out,
                            false,
                        )
                        .await;
                    }

                    if let Some(ref u) = usage {
                        let _ = chat::upsert_usage(
                            &pool_clone,
                            session_id,
                            u.input_tokens as i64,
                            u.output_tokens as i64,
                            u.cache_read_tokens as i64,
                        )
                        .await;
                    }

                    let _ = app.emit(
                        "chat:done",
                        ChatDoneEvent {
                            session_id,
                            usage,
                            stop_reason,
                        },
                    );
                    break;
                }
                IncomingMessage::Error {
                    category, message, ..
                } => {
                    if !current_tool_ids.is_empty() {
                        segments.push(serde_json::json!({"type": "tool_group", "tool_use_ids": current_tool_ids}));
                    }
                    if !current_text.is_empty() {
                        segments.push(serde_json::json!({"type": "text", "text": current_text}));
                    }
                    if !segments.is_empty() {
                        let content = serde_json::json!(&segments);
                        let _ = chat::insert_message(
                            &pool_clone,
                            session_id,
                            "assistant",
                            &content.to_string(),
                            None,
                            None,
                            true,
                        )
                        .await;
                    }

                    let _ = app.emit(
                        "chat:error",
                        ChatErrorEvent {
                            session_id,
                            category,
                            message,
                        },
                    );
                    break;
                }
                IncomingMessage::Pong => {}
            }
        }

        if let Some(mgr) = sidecar_clone.lock().await.as_ref() {
            mgr.unregister_request(&request_id_clone).await;
        }
    });

    Ok(request_id)
}

/// Cancel an in-flight chat request mid-stream. The sidecar reacts by
/// sending a final `chat:done` with `cancelled: true`.
#[tauri::command]
pub async fn abort_chat(
    sidecar: tauri::State<'_, SidecarState>,
    request_id: String,
) -> Result<(), String> {
    let guard = sidecar.lock().await;
    let mgr = guard.as_ref().ok_or("Sidecar not initialized")?;
    mgr.send(OutgoingMessage::Abort { request_id }).await
}

/// Resolve a `PermissionRequest` from the sidecar (Allow / Deny with
/// scope Once / Session / Always). For Session and Always scopes,
/// also persists a rule in `tool_permissions` so future identical
/// requests skip the user prompt.
#[tauri::command]
pub async fn respond_chat_permission(
    pool: tauri::State<'_, SqlitePool>,
    sidecar: tauri::State<'_, SidecarState>,
    permission_id: String,
    behavior: String,
    scope: Option<String>,
    tool_name: Option<String>,
    message: Option<String>,
) -> Result<(), String> {
    let decision_behavior = match behavior.as_str() {
        "allow" => PermissionBehavior::Allow,
        "deny" => PermissionBehavior::Deny,
        _ => return Err(format!("Invalid behavior: {behavior}")),
    };

    let scope_str = scope.as_deref().unwrap_or("once");
    if let (true, Some(tn)) = (
        scope_str == "session" || scope_str == "always",
        tool_name.as_deref(),
    ) {
        let _ = chat::insert_permission(
            &pool, tn, None, // path_pattern — generic rule
            None, // workspace — global
            scope_str, &behavior,
            None, // session_id — not needed for 'always', and for 'session' we match by tool_name
        )
        .await;
    }

    let guard = sidecar.lock().await;
    let mgr = guard.as_ref().ok_or("Sidecar not initialized")?;
    mgr.send(OutgoingMessage::PermissionResponse {
        permission_id,
        decision: PermissionDecision {
            behavior: decision_behavior,
            message,
        },
    })
    .await
}

/// List persisted permission rules for a workspace (or global rules
/// when `workspace` is `None`).
#[tauri::command]
pub async fn list_tool_permissions(
    pool: tauri::State<'_, SqlitePool>,
    workspace: Option<String>,
) -> Result<Vec<chat::PermissionRule>, String> {
    chat::list_permissions(&pool, workspace.as_deref()).await
}

/// Remove a persisted permission rule by id (PermissionManager UI).
#[tauri::command]
pub async fn delete_tool_permission(
    pool: tauri::State<'_, SqlitePool>,
    id: i64,
) -> Result<(), String> {
    chat::delete_permission(&pool, id).await
}

/// Persist a clipboard / dropped image to the app's tmp dir and
/// return the absolute path. Used to keep the in-flight chat IPC
/// payload light — the frontend only needs to send the path.
#[tauri::command]
pub async fn save_attachment_tmp(bytes: Vec<u8>, media_type: String) -> Result<String, String> {
    let ext = match media_type.as_str() {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "bin",
    };

    let tmp_dir = httui_core::paths::default_data_dir()
        .map_err(|e| format!("Failed to resolve data dir: {e}"))?
        .join("tmp");

    tokio::fs::create_dir_all(&tmp_dir)
        .await
        .map_err(|e| format!("Failed to create tmp dir: {e}"))?;

    let filename = format!("{}.{}", Uuid::new_v4(), ext);
    let path = tmp_dir.join(&filename);

    tokio::fs::write(&path, &bytes)
        .await
        .map_err(|e| format!("Failed to write tmp file: {e}"))?;

    Ok(path.to_string_lossy().to_string())
}

/// Delete every message in `session_id` whose `turn_index >=
/// turn_index`. Used by the "edit and resend" flow to truncate the
/// transcript before re-sending.
#[tauri::command]
pub async fn delete_messages_after(
    pool: tauri::State<'_, SqlitePool>,
    session_id: i64,
    turn_index: i64,
) -> Result<(), String> {
    sqlx::query("DELETE FROM messages WHERE session_id = ? AND turn_index >= ?")
        .bind(session_id)
        .bind(turn_index)
        .execute(pool.inner())
        .await
        .map_err(|e| format!("Failed to delete messages: {e}"))?;
    Ok(())
}

/// Drop the stored `claude_session_id` so the next `send_chat_message`
/// starts a fresh Claude session (used after a resume failure).
#[tauri::command]
pub async fn clear_session_claude_id(
    pool: tauri::State<'_, SqlitePool>,
    session_id: i64,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE sessions SET claude_session_id = NULL, updated_at = unixepoch() WHERE id = ?",
    )
    .bind(session_id)
    .execute(pool.inner())
    .await
    .map_err(|e| format!("Failed to clear claude_session_id: {e}"))?;
    Ok(())
}

/// Change the working directory associated with a session. The new
/// `cwd` takes effect on the next `send_chat_message`.
#[tauri::command]
pub async fn update_chat_session_cwd(
    pool: tauri::State<'_, SqlitePool>,
    session_id: i64,
    cwd: Option<String>,
) -> Result<(), String> {
    sqlx::query("UPDATE sessions SET cwd = ?, updated_at = unixepoch() WHERE id = ?")
        .bind(&cwd)
        .bind(session_id)
        .execute(pool.inner())
        .await
        .map_err(|e| format!("Failed to update session cwd: {e}"))?;
    Ok(())
}

/// Return token usage aggregated per day in the inclusive `[from, to]`
/// range (ISO `YYYY-MM-DD`). Powers the `UsagePanel` chart.
#[tauri::command]
pub async fn get_usage_stats(
    pool: tauri::State<'_, SqlitePool>,
    from: String,
    to: String,
) -> Result<Vec<chat::DailyUsage>, String> {
    chat::get_usage_by_date_range(&pool, &from, &to).await
}

fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Resolve a wikilink target to note content by searching the vault directory.
/// Matches files by stem (filename without extension), case-insensitive.
fn resolve_wikilink(vault_path: &str, target: &str) -> Result<String, String> {
    let target_lower = target.to_lowercase();
    let vault = std::path::Path::new(vault_path);

    fn find_note(dir: &std::path::Path, target: &str) -> Option<std::path::PathBuf> {
        let entries = std::fs::read_dir(dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Skip common heavy directories
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with('.') || name_str == "node_modules" || name_str == "target" {
                    continue;
                }
                if let Some(found) = find_note(&path, target) {
                    return Some(found);
                }
            } else if let Some(ext) = path.extension() {
                if ext == "md" {
                    if let Some(stem) = path.file_stem() {
                        if stem.to_string_lossy().to_lowercase() == target {
                            return Some(path);
                        }
                    }
                }
            }
        }
        None
    }

    let note_path =
        find_note(vault, &target_lower).ok_or_else(|| format!("Note not found: {target}"))?;

    std::fs::read_to_string(&note_path)
        .map_err(|e| format!("Failed to read note {}: {e}", note_path.display()))
}

const MAX_IMAGE_DIMENSION: u32 = 2048;

/// Normalize an image: resize if either side > 2048px, re-encode as JPEG Q85.
/// Returns (normalized_bytes, media_type). Passes through unchanged if already small enough and JPEG.
fn normalize_image(bytes: &[u8], media_type: &str) -> Result<(Vec<u8>, String), String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("Failed to decode image: {e}"))?;

    let (w, h) = (img.width(), img.height());
    let needs_resize = w > MAX_IMAGE_DIMENSION || h > MAX_IMAGE_DIMENSION;
    let is_jpeg = media_type == "image/jpeg";

    if !needs_resize && is_jpeg {
        return Ok((bytes.to_vec(), media_type.to_string()));
    }

    let img = if needs_resize {
        img.resize(
            MAX_IMAGE_DIMENSION,
            MAX_IMAGE_DIMENSION,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        img
    };

    let mut buf = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 85);
    img.to_rgb8()
        .write_with_encoder(encoder)
        .map_err(|e| format!("Failed to encode JPEG: {e}"))?;

    Ok((buf, "image/jpeg".to_string()))
}
