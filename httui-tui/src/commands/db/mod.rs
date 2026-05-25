//! DB block domain commands and helpers.
//!
//! Pulled out of `vim::dispatch` so the vim layer stops growing
//! database-specific logic. Today this module owns:
//! - SQL query classification helpers (`is_cacheable_query`,
//!   `is_writing_query`, `is_unscoped_destructive`)
//! - Cache key derivation (`compute_db_cache_hash`) and async save
//! - The on-screen status formatter for cached results
//!   (`db_summary_from_value`)
//! - Connection slug → UUID resolver (`resolve_connection_id_sync`)
//! - Ref / bind resolution (`resolve_block_refs` and friends)
//! - Env vars + connection lookup (`load_active_env_vars`,
//!   `resolve_connection_id`)
//! - Executor params builder + response summary
//!   (`build_db_executor_params`, `summarize_db_response`)
//! - The full block-execution flow (`apply_run_block`,
//!   `run_db_block_inner`, `spawn_db_query`,
//!   `handle_db_block_result`, `cancel_running_query`,
//!   `load_more_db_block`)
//! - The EXPLAIN entry point (`run_explain`, bound to `<C-x>`)

use crate::app::{App, StatusKind};
use crate::buffer::{Cursor, Segment};

mod export_picker;
mod history;
mod result;
mod run;
mod settings_modal;
mod sql;
pub use export_picker::*;
pub use history::*;
pub use result::*;
pub use run::*;
pub use settings_modal::*;
pub use sql::*;


/// Resolve a fence's `connection=` value (UUID or slug) to the
/// canonical UUID using the in-memory `connection_names` map. The
/// async `resolve_connection_id` (used by the executor) hits the
/// SQLite pool and we can't await on every keystroke; the names map
/// is loaded at startup and refreshed after CRUD, so a sync scan
/// is enough for popup-time lookups.
///
/// Returns the input verbatim when neither a key nor a value
/// matches — that way an unknown id still flows through and
/// `schema_cache.get(...)` simply yields `None`.
pub fn resolve_connection_id_sync(
    raw: &str,
    names: &std::collections::HashMap<String, String>,
) -> String {
    if names.contains_key(raw) {
        return raw.to_string();
    }
    for (id, name) in names {
        if name.eq_ignore_ascii_case(raw) {
            return id.clone();
        }
    }
    raw.to_string()
}

/// `gd` — cycle the focused block's `display_mode` (Input → Split
/// → Output → Input) and persist the new value into the fence so
/// the choice survives save/reload. Snapshots the document for
/// undo. Works on any block type (HTTP / DB / E2E) since
/// `display_mode` lives on `BlockNode`, but the renderer only
/// honors it for DB today (HTTP/E2E render single-mode for now).
pub fn cycle_display_mode(app: &mut App) {
    let segment_idx = match app.document().map(|d| d.cursor()) {
        Some(Cursor::InBlock { segment_idx, .. })
        | Some(Cursor::InBlockResult { segment_idx, .. }) => segment_idx,
        _ => {
            app.set_status(StatusKind::Info, "place the cursor on a block first");
            return;
        }
    };
    let next = {
        let Some(doc) = app.tabs.active_document_mut() else {
            return;
        };
        // Snapshot before mutating so undo can roll the mode back.
        // The mode lives on `BlockNode.display_mode`, which the
        // snapshot already captures along with the rest of the
        // segment slice.
        doc.snapshot();
        let Some(block) = doc.block_at_mut(segment_idx) else {
            return;
        };
        let next = block.effective_display_mode().next();
        block.display_mode = Some(next.as_str().to_string());
        next
    };
    app.set_status(StatusKind::Info, format!("display: {}", next.as_str()));
}

/// `<C-a>` — open the inline alias-edit prompt for the block at the
/// cursor. Prefills with the current alias so the user can either
/// just hit Enter (no-op), edit it, or wipe it (Backspace clears →
/// Enter commits a blank alias, making the block anonymous). The
/// edit only commits via `confirm_fence_edit`; cancelling leaves
/// the alias untouched.
pub fn open_fence_edit_alias(app: &mut App) {
    let segment_idx = match app.document().map(|d| d.cursor()) {
        Some(Cursor::InBlock { segment_idx, .. })
        | Some(Cursor::InBlockResult { segment_idx, .. }) => segment_idx,
        _ => {
            app.set_status(StatusKind::Info, "place the cursor on a block first");
            return;
        }
    };
    let prefill = match app.document().and_then(|d| d.segments().get(segment_idx)) {
        Some(Segment::Block(b)) => b.alias.clone().unwrap_or_default(),
        _ => {
            app.set_status(StatusKind::Info, "no block at cursor");
            return;
        }
    };
    app.modal = Some(crate::modal::Modal::Prompt(
        crate::modal::PromptKind::FenceEditAlias { segment_idx },
        crate::vim::lineedit::LineEdit::from_str(prefill),
    ));
    app.vim.mode = crate::vim::mode::Mode::FenceEdit;
    app.vim.reset_pending();
}

/// `<CR>` inside the fence-edit prompt — validate + commit the edit
/// to the block. Today only the alias kind exists; once limit /
/// timeout slices land, the per-kind validation lives in this match.
/// On success: snapshot the doc (so undo can roll back), write the
/// new value, drop the prompt, return to normal mode. On failure:
/// the prompt stays open and the status bar surfaces the reason.
pub fn confirm_fence_edit(app: &mut App) {
    let Some((crate::modal::PromptKind::FenceEditAlias { segment_idx }, le)) =
        app.modal.as_ref().and_then(|m| m.as_prompt())
    else {
        app.vim.enter_normal();
        return;
    };
    let raw = le.as_str().trim().to_string();

    // Empty input = clear the alias (block becomes anonymous —
    // valid; the block just stops being referencable as
    // `{{alias.path}}`).
    let new_alias: Option<String> = if raw.is_empty() {
        None
    } else {
        let dup = app
            .document()
            .and_then(|d| validate_alias_unique(d, segment_idx, &raw).err());
        if let Some(msg) = dup {
            app.set_status(StatusKind::Error, msg);
            return;
        }
        Some(raw)
    };
    let Some(doc) = app.tabs.active_document_mut() else {
        return;
    };
    doc.snapshot();
    if let Some(block) = doc.block_at_mut(segment_idx) {
        // Mirror `block.alias` into `params.alias` so the
        // serializer's roundtrip stays lossless (the parser
        // reads from the info-string token, but other code
        // paths read from `params`).
        block.alias = new_alias.clone();
        if let Some(obj) = block.params.as_object_mut() {
            match &new_alias {
                Some(a) => {
                    obj.insert("alias".into(), serde_json::Value::String(a.clone()));
                }
                None => {
                    obj.remove("alias");
                }
            }
        }
    }

    app.modal = None;
    app.vim.enter_normal();
    app.set_status(StatusKind::Info, "alias updated");
}

/// Reject duplicate aliases inside the same document. `{{alias.path}}`
/// resolution walks blocks-above-current looking for the first matching
/// alias, so two blocks with the same alias would silently shadow
/// each other and hide the second's results from refs. Loud failure
/// at edit time is the better experience.
///
/// Pure (`&Document` not `&App`) so it lives behind `cargo test`
/// without an `App` fixture.
fn validate_alias_unique(
    doc: &crate::buffer::Document,
    segment_idx: usize,
    candidate: &str,
) -> Result<(), String> {
    for (idx, seg) in doc.segments().iter().enumerate() {
        if idx == segment_idx {
            continue;
        }
        if let Segment::Block(b) = seg {
            if b.alias.as_deref() == Some(candidate) {
                return Err(format!(
                    "alias `{candidate}` already used by another block in this doc"
                ));
            }
        }
    }
    Ok(())
}

/// `<C-x>` — wrap the focused DB block's query in the dialect's
/// EXPLAIN keyword and run it. The block's own query text stays
/// untouched (override flows only to the executor); the explain
/// output lands in the block's `cached_result` like any other run.
pub fn run_explain(app: &mut App) {
    let Some(doc) = app.document() else { return };
    let segment_idx = match doc.cursor() {
        Cursor::InBlock { segment_idx, .. } => segment_idx,
        Cursor::InBlockResult { segment_idx, .. } => segment_idx,
        _ => {
            app.set_status(StatusKind::Info, "place the cursor on a DB block first");
            return;
        }
    };
    let block = match doc.segments().get(segment_idx) {
        Some(Segment::Block(b)) => b.clone(),
        _ => return,
    };
    if !block.is_db() {
        app.set_status(StatusKind::Info, "not a DB block");
        return;
    }
    let raw = block
        .params
        .get("query")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let dialect = crate::sql_completion::Dialect::from_block(&block);
    let wrapped = crate::sql_completion::explain_wrap(raw, dialect);
    run_db_block_inner(
        app,
        segment_idx,
        /* force_unscoped = */ true,
        Some(wrapped),
        /* as_explain = */ true,
    );
}

// ───────────── ref / bind resolution ─────────────

/// Replace `{{alias.response.path...}}` placeholders in `query` with
/// SQL bind placeholders (`?`) and collect each resolved value into a
/// parallel array. Mirrors `resolveRefsToBindParams` on the desktop
/// (`src/components/blocks/db/fenced/DbFencedPanel.tsx:340-360`):
/// values **never** become part of the SQL string — sqlx binds them
/// at the driver layer, so a malicious upstream value like
/// `'7; DROP TABLE x'` lands as a single literal string parameter,
/// not as injected SQL.
///
/// The function is pure: callers thread the document's segment slice
/// in. That keeps tests free of `App` plumbing and matches how
/// `apply_run_block` / `load_more_db_block` already split the
/// pre-flight (read-only) phase from the spawn (mutates `app`).
pub fn resolve_block_refs(
    segments: &[crate::buffer::Segment],
    current_segment: usize,
    query: &str,
    env_vars: &std::collections::HashMap<String, String>,
) -> Result<(String, Vec<serde_json::Value>), String> {
    let mut out = String::with_capacity(query.len());
    let mut binds: Vec<serde_json::Value> = Vec::new();
    let bytes = query.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        // `{{` opens a placeholder. Anything else is copied verbatim.
        if i + 1 < bytes.len() && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            let close = match find_close_marker(&bytes[i + 2..]) {
                Some(rel) => i + 2 + rel,
                None => {
                    out.push('{');
                    i += 1;
                    continue;
                }
            };
            let inner = std::str::from_utf8(&bytes[i + 2..close])
                .map_err(|_| "invalid utf-8 inside reference".to_string())?
                .trim();
            let value = resolve_one_ref(segments, current_segment, inner, env_vars)?;
            out.push('?');
            binds.push(value);
            i = close + 2;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    Ok((out, binds))
}

/// Locate the `}}` closing brace inside a placeholder body. Returns
/// `None` if the placeholder is never closed (the caller falls back
/// to copying the input).
fn find_close_marker(b: &[u8]) -> Option<usize> {
    let mut i = 0usize;
    while i + 1 < b.len() {
        if b[i] == b'}' && b[i + 1] == b'}' {
            return Some(i);
        }
        i += 1;
    }
    None
}

pub(crate) fn resolve_one_ref(
    segments: &[crate::buffer::Segment],
    current_segment: usize,
    inner: &str,
    env_vars: &std::collections::HashMap<String, String>,
) -> Result<serde_json::Value, String> {
    let parts: Vec<&str> = inner.split('.').map(str::trim).collect();
    let head = parts.first().copied().unwrap_or("").trim();
    if head.is_empty() {
        return Err("empty reference".into());
    }
    // Block refs are dotted (`alias.field…`); when missing, fall back
    // to env vars only for single-segment keys (`{{TOKEN}}`). This
    // mirrors the desktop precedence: blocks win over env collisions.
    let block_match = segments
        .iter()
        .take(current_segment)
        .filter_map(|s| match s {
            crate::buffer::Segment::Block(b) => Some(b),
            _ => None,
        })
        .find(|b| b.alias.as_deref() == Some(head));
    if let Some(block) = block_match {
        let cached = block
            .cached_result
            .as_ref()
            .ok_or_else(|| format!("block `{head}` hasn't run yet — execute it first"))?;
        let nav: Vec<&str> = parts[1..].to_vec();

        // DB blocks get the multi-result shim that mirrors desktop's
        // `makeDbResponseView` (`src/lib/blocks/references.ts:174-223`):
        // the `response.*` namespace exposes three access patterns
        // (passthrough / numeric / legacy column). Non-DB blocks keep
        // the simple "strip `response` and dot-navigate" behavior.
        if block.is_db() && nav.first().copied() == Some("response") && is_db_response_shape(cached)
        {
            return resolve_db_response_path(cached, &nav[1..]);
        }

        // Skip a literal `response` segment for desktop-compat:
        // `{{alias.response.path}}` ≡ `{{alias.path}}`.
        let mut nav = nav;
        if nav.first().copied() == Some("response") {
            nav.remove(0);
        }
        let mut value = cached;
        for part in &nav {
            value = navigate_json(value, part)
                .ok_or_else(|| format!("path `{part}` not found in `{head}`"))?;
        }
        return value_for_bind(value);
    }
    // No matching block. A dotted reference can only be a block, so
    // fail loudly. Single-segment refs try env vars next.
    if parts.len() > 1 {
        return Err(format!("block `{head}` not found above this one"));
    }
    if let Some(v) = env_vars.get(head) {
        // Env values bind as plain strings — same shape every other
        // value gets, so the driver decides numeric coercion.
        return Ok(serde_json::Value::String(v.clone()));
    }
    Err(format!("`{head}` is not a block alias above or an env var"))
}

/// Quick check: does this cached value carry the shape of a serialized
/// `DbResponse` (top-level `results` array)? Used to gate the DB-only
/// ref shim so older / non-DB cached blobs keep navigating raw.
fn is_db_response_shape(v: &serde_json::Value) -> bool {
    v.get("results").map(|r| r.is_array()).unwrap_or(false)
}

/// Navigate the part of a `{{alias.response.…}}` ref that comes
/// *after* the literal `response` segment. Three access patterns,
/// dispatched on the first remaining segment:
///
/// - `response.results` / `response.messages` / `response.stats` /
///   `response.plan` — passthrough to the matching `DbResponse` field.
/// - `response.<N>` — numeric shortcut for `results[N]`.
/// - `response.<col>` — legacy shim from before multi-result existed:
///   the column is read from `results[0].rows[0]`.
fn resolve_db_response_path(
    cached: &serde_json::Value,
    nav: &[&str],
) -> Result<serde_json::Value, String> {
    // `{{alias.response}}` alone — there's nothing scalar to bind.
    let Some((first, rest)) = nav.split_first() else {
        return Err("reference points to a non-scalar value".into());
    };

    // Passthrough fields — `response.results`, `response.stats`, etc.
    // We let the user navigate *through* these the long way: it's the
    // shape `{{` autocomplete will guide users toward.
    if matches!(*first, "results" | "messages" | "stats" | "plan") {
        let mut value = cached
            .get(*first)
            .ok_or_else(|| format!("response has no `{first}`"))?;
        for part in rest {
            value = navigate_json(value, part).ok_or_else(|| format!("path `{part}` not found"))?;
        }
        return value_for_bind(value);
    }

    let results = cached
        .get("results")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "response has no results array".to_string())?;

    // Numeric shortcut: `response.0.rows.0.id` ≡ `response.results.0.rows.0.id`.
    if let Ok(idx) = first.parse::<usize>() {
        let mut value = results.get(idx).ok_or_else(|| {
            format!(
                "result index {idx} out of bounds (have {} result(s))",
                results.len()
            )
        })?;
        for part in rest {
            value = navigate_json(value, part).ok_or_else(|| format!("path `{part}` not found"))?;
        }
        return value_for_bind(value);
    }

    // Legacy column shim: `response.col` → `results[0].rows[0].col`.
    // The pre-redesign refs all looked like this — keep them working
    // so existing notes don't break.
    let first_result = results
        .first()
        .ok_or_else(|| "response has no result sets".to_string())?;
    let rows = first_result
        .get("rows")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "first result has no rows (was it a mutation or error?)".to_string())?;
    let first_row = rows
        .first()
        .ok_or_else(|| "first result has no rows yet".to_string())?;
    let mut value = navigate_json(first_row, first)
        .ok_or_else(|| format!("column `{first}` not found in first row"))?;
    for part in rest {
        value = navigate_json(value, part).ok_or_else(|| format!("path `{part}` not found"))?;
    }
    value_for_bind(value)
}

fn navigate_json<'a>(v: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    if let Ok(idx) = key.parse::<usize>() {
        if let Some(arr) = v.as_array() {
            return arr.get(idx);
        }
    }
    v.as_object()?.get(key)
}

/// Verify a reference's resolved value is bind-safe and clone it for
/// the bind array. Arrays and objects can't go through driver-side
/// parameter binding for the dialects we target, so reject them
/// loudly; the user almost always meant a scalar field anyway and a
/// silent JSON-stringify would mask the typo.
fn value_for_bind(v: &serde_json::Value) -> Result<serde_json::Value, String> {
    match v {
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => Ok(v.clone()),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            Err("reference points to a non-scalar value".into())
        }
    }
}

// ───────────── env vars / connection lookups ─────────────

/// Load the active environment's variables into a `key → value` map.
/// Uses `list_vars_resolved` so secrets come through the keychain —
/// `list_vars` would hand back empty values for every secret, which
/// would silently turn `{{TOKEN}}` into the empty string at request
/// dispatch. Returns `None` when there's no active env, the lookup
/// fails, or the env has no vars — callers fall back to an empty map.
pub async fn load_active_env_vars(
    store: &httui_core::vault_config::EnvironmentsStore,
) -> Option<std::collections::HashMap<String, String>> {
    let env_name = store.active_env().await.ok().flatten()?;
    let vars = store.list_vars_resolved(&env_name).await.ok()?;
    Some(vars.into_iter().map(|v| (v.key, v.value)).collect())
}

/// Resolve a fence's `connection=` value to the connection key the
/// pool manager / executor expect. V3 reordering 2026-05-23: the
/// `ConnectionsStore` keys connections by name (TOML row key), so
/// "id" and "name" are the same string. A successful lookup just
/// returns the name back; the only job left is producing a clear
/// "not found" when the block references something that's not in the
/// vault's `connections.toml`.
pub async fn resolve_connection_id(
    store: &httui_core::vault_config::ConnectionsStore,
    key: &str,
) -> Result<String, String> {
    if store
        .get(key)
        .await
        .map_err(|e| format!("connection lookup failed: {e}"))?
        .is_some()
    {
        return Ok(key.to_string());
    }
    Err(format!("Connection '{key}' not found"))
}




#[cfg(test)]
mod tests;
