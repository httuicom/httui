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

use tokio_util::sync::CancellationToken;

use crate::app::{App, StatusKind};
use crate::buffer::block::ExecutionState;
use crate::buffer::{Cursor, Segment};

/// Strip leading whitespace + line / block comments so query
/// classifiers see the first *real* statement word. Shared between
/// `is_cacheable_query`, `is_writing_query`, and `is_unscoped_destructive`.
pub fn strip_leading_sql_comments(query: &str) -> &str {
    let mut s = query.trim_start();
    loop {
        if let Some(rest) = s.strip_prefix("--") {
            s = match rest.find('\n') {
                Some(idx) => rest[idx + 1..].trim_start(),
                None => "",
            };
        } else if let Some(rest) = s.strip_prefix("/*") {
            s = match rest.find("*/") {
                Some(idx) => rest[idx + 2..].trim_start(),
                None => "",
            };
        } else {
            break;
        }
    }
    s
}

/// Decide whether a query is safe to serve from cache. Read-only
/// statements (SELECT/EXPLAIN/WITH/SHOW/PRAGMA/DESC) cache; anything
/// else (UPDATE/DELETE/INSERT/DDL) bypasses the cache and always
/// re-executes — matching desktop semantics.
pub fn is_cacheable_query(query: &str) -> bool {
    let s = strip_leading_sql_comments(query);
    let first_word: String = s.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    matches!(
        first_word.to_ascii_uppercase().as_str(),
        "SELECT" | "WITH" | "EXPLAIN" | "SHOW" | "PRAGMA" | "DESC" | "DESCRIBE"
    )
}

/// Whether the query writes to the database. The read-only gate
/// uses this to decide if a query against an `is_readonly`
/// connection should be blocked. Strict list — anything not
/// recognized as a write counts as a read (safer default for the
/// gate: we'd rather let a weird read through than block one).
pub fn is_writing_query(query: &str) -> bool {
    let s = strip_leading_sql_comments(query);
    let first_word: String = s.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    matches!(
        first_word.to_ascii_uppercase().as_str(),
        "UPDATE"
            | "DELETE"
            | "INSERT"
            | "REPLACE"
            | "MERGE"
            | "CREATE"
            | "DROP"
            | "ALTER"
            | "TRUNCATE"
            | "GRANT"
            | "REVOKE"
            | "VACUUM"
    )
}

/// Whether the query is an `UPDATE` or `DELETE` *without* a `WHERE`
/// clause — the kind of slip that nukes an entire table. Used by
/// the confirm gate.
pub fn is_unscoped_destructive(query: &str) -> bool {
    let s = strip_leading_sql_comments(query);
    let first_word: String = s.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let kind = first_word.to_ascii_uppercase();
    if kind != "UPDATE" && kind != "DELETE" {
        return false;
    }
    let stmt_end = s.find(';').unwrap_or(s.len());
    let stmt = &s[..stmt_end];
    let upper = stmt.to_ascii_uppercase();
    let mut start = 0;
    while let Some(pos) = upper[start..].find("WHERE") {
        let abs = start + pos;
        let before_ok = abs == 0
            || !upper.as_bytes()[abs - 1].is_ascii_alphanumeric()
                && upper.as_bytes()[abs - 1] != b'_';
        let after = abs + 5;
        let after_ok = after >= upper.len()
            || (!upper.as_bytes()[after].is_ascii_alphanumeric()
                && upper.as_bytes()[after] != b'_');
        if before_ok && after_ok {
            return false;
        }
        start = abs + 5;
    }
    true
}

/// Build the cache hash for a DB block run. Mirrors desktop's
/// `computeDbCacheHash`: hash text is the raw SQL body plus, when
/// any env vars are referenced via `{{KEY}}`, a sorted `KEY=VALUE`
/// snapshot of just those vars. Connection id goes in as a separate
/// hash input so the same query against two connections can't
/// collide. Stays in lockstep with the desktop so both apps' caches
/// share entries when querying the same vault.
pub fn compute_db_cache_hash(
    body: &str,
    conn_id: Option<&str>,
    env_vars: &std::collections::HashMap<String, String>,
) -> String {
    let mut used: Vec<(&String, &String)> = env_vars
        .iter()
        .filter(|(k, _)| body.contains(&format!("{{{{{k}}}}}")))
        .collect();
    used.sort_by(|a, b| a.0.cmp(b.0));
    let env_block: String = used
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("\n");
    let keyed = if env_block.is_empty() {
        body.to_string()
    } else {
        format!("{body}\n__ENV__\n{env_block}")
    };
    httui_core::block_results::compute_block_hash(&keyed, None, conn_id)
}

/// Format the same one-liner `db_summary` produces in the renderer
/// — but driven by an arbitrary `Value` (the deserialized cache
/// row) rather than a `BlockNode`. Used to paint the `⛁ cached · …`
/// status when a cache hit short-circuits the run. Errors with
/// position get an ` at L:C` suffix matching `summarize_db_response`.
pub fn db_summary_from_value(value: Option<&serde_json::Value>, elapsed: u64) -> String {
    let Some(v) = value else {
        return format!("ok · {elapsed}ms");
    };
    let results = v.get("results").and_then(|r| r.as_array());
    let extras = match results.map(|r| r.len()).unwrap_or(0) {
        0 | 1 => String::new(),
        n => format!(" (+{} more)", n - 1),
    };
    let first = results.and_then(|r| r.first());
    let kind = first.and_then(|f| f.get("kind")).and_then(|k| k.as_str());
    match kind {
        Some("select") => {
            let rows = first
                .and_then(|f| f.get("rows"))
                .and_then(|r| r.as_array())
                .map(|r| r.len())
                .unwrap_or(0);
            let has_more = first
                .and_then(|f| f.get("has_more"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let suffix = if has_more { "+" } else { "" };
            format!("{rows}{suffix} rows · {elapsed}ms{extras}")
        }
        Some("mutation") => {
            let affected = first
                .and_then(|f| f.get("rows_affected"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            format!("{affected} affected · {elapsed}ms{extras}")
        }
        Some("error") => first
            .and_then(|f| f.get("message"))
            .and_then(|v| v.as_str())
            .map(|m| {
                let pos = first
                    .and_then(|f| f.get("line"))
                    .and_then(|l| l.as_u64())
                    .map(|line| {
                        let col = first
                            .and_then(|f| f.get("column"))
                            .and_then(|c| c.as_u64())
                            .unwrap_or(1);
                        format!(" at {line}:{col}")
                    })
                    .unwrap_or_default();
                format!("error: {m}{pos}{extras}")
            })
            .unwrap_or_else(|| format!("error · {elapsed}ms")),
        _ => format!("ok · {elapsed}ms{extras}"),
    }
}

/// Fire-and-forget save to the on-disk cache. Spawned because the
/// SQLite write would otherwise block the dispatcher; failure is
/// logged but never surfaces to the user (cache writes are
/// best-effort, matching the desktop). Pulls `total_rows` from the
/// first SELECT result so the cached row matches desktop's shape.
pub fn save_db_cache_async(
    pool: sqlx::SqlitePool,
    file_path: String,
    hash: String,
    value: serde_json::Value,
    elapsed_ms: u64,
    results: &[httui_core::executor::db::types::DbResult],
) {
    use httui_core::executor::db::types::DbResult;
    let total_rows: Option<i64> = results.first().and_then(|r| match r {
        DbResult::Select { rows, .. } => Some(rows.len() as i64),
        _ => None,
    });
    let response_str = match serde_json::to_string(&value) {
        Ok(s) => s,
        Err(_) => return,
    };
    tokio::spawn(async move {
        let _ = httui_core::block_results::save_block_result(
            &pool,
            &file_path,
            &hash,
            "success",
            &response_str,
            elapsed_ms as i64,
            total_rows,
        )
        .await;
    });
}

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

// ───────────── executor params + response summary ─────────────

/// Assemble the JSON `serde_json::Value` that `DbExecutor`
/// deserializes into its `DbParams`. Pure function — extracted from
/// `spawn_db_query` so it's testable in isolation. Stays in lockstep
/// with `httui_core::executor::db::DbParams`: any new field there
/// has to be threaded through here.
pub fn build_db_executor_params(
    connection_id: &str,
    query: &str,
    bind_values: &[serde_json::Value],
    offset: u64,
    limit: u64,
    timeout_ms: Option<u64>,
) -> serde_json::Value {
    serde_json::json!({
        "connection_id": connection_id,
        "query": query,
        "bind_values": bind_values,
        "offset": offset,
        "fetch_size": limit,
        // `timeout_ms` is `Option<u64>`; serde maps `None` → `null`
        // → executor's `Option<u64>` deserializes back to `None`,
        // which falls through to the connection's default timeout
        // and ultimately the 30 s fallback in `execute_with_cancel`.
        "timeout_ms": timeout_ms,
    })
}

/// Compact one-liner for the status bar: `5 rows · 12ms` /
/// `mutation: 3 affected · 8ms` / `error: …`. Multi-statement
/// queries get a `(+N more)` suffix so users know the renderer is
/// only surfacing `results[0]` for now (ships tabs).
pub fn summarize_db_response(resp: &httui_core::executor::db::types::DbResponse) -> String {
    use httui_core::executor::db::types::DbResult;
    let elapsed = resp.stats.elapsed_ms;
    let extras = match resp.results.len() {
        0 | 1 => String::new(),
        n => format!(" (+{} more)", n - 1),
    };
    if let Some(first) = resp.results.first() {
        match first {
            DbResult::Select { rows, has_more, .. } => {
                let suffix = if *has_more { "+" } else { "" };
                format!("{}{} rows · {}ms{}", rows.len(), suffix, elapsed, extras)
            }
            DbResult::Mutation { rows_affected } => {
                format!("{} affected · {}ms{}", rows_affected, elapsed, extras)
            }
            DbResult::Error {
                message,
                line,
                column,
            } => {
                // Append `at L:C` when the executor enriched the
                // error with positional info (Postgres always; MySQL
                // when the parser produced one). Same suffix the
                // renderer's `db_summary` paints inside the block.
                let pos = line
                    .map(|l| format!(" at {l}:{}", column.unwrap_or(1)))
                    .unwrap_or_default();
                format!("error: {message}{pos}{extras}")
            }
        }
    } else {
        format!("ok · {}ms", elapsed)
    }
}

// ───────────── block execution (`r` in normal) ─────────────

/// Run the block at the cursor. Phase 1 only handles `db` / `db-*`
/// blocks — everything else surfaces a status hint and bails. The
/// query runs in a `tokio::spawn` task so the UI stays responsive
/// (and `Ctrl-C` can cancel it via the stored `CancellationToken`).
/// When the task finishes it pushes an `AppEvent::DbBlockResult`
/// back to the main loop, which folds the outcome into the block
/// via `handle_db_block_result`.
pub fn apply_run_block(app: &mut App) {
    if app.running_query.is_some() {
        app.set_status(
            StatusKind::Info,
            "another block is already running — Ctrl-C to cancel",
        );
        return;
    }

    let Some(doc) = app.document() else { return };
    let Cursor::InBlock { segment_idx, .. } = doc.cursor() else {
        app.set_status(
            StatusKind::Info,
            "no block at cursor (place cursor on a block first)",
        );
        return;
    };
    let block_type = match doc.segments().get(segment_idx) {
        Some(Segment::Block(b)) => b.block_type.clone(),
        _ => return,
    };

    if block_type == "http" {
        crate::commands::http::apply_run_http_block(app, segment_idx);
        return;
    }
    if block_type.starts_with("db-") || block_type == "db" {
        run_db_block_inner(
            app,
            segment_idx,
            /* force_unscoped = */ false,
            None,
            /* as_explain = */ false,
        );
        return;
    }
    app.set_status(
        StatusKind::Info,
        format!("`{block_type}` blocks aren't runnable yet"),
    );
}

/// Run the DB block at `segment_idx`. Shared entry for the
/// cursor-based `r` keypress, the confirm-modal `y`, and `<C-x>`.
/// The `force_unscoped` flag bypasses the unscoped-destructive gate
/// once — set only when the user explicitly confirmed the run, or
/// when the call is internal (EXPLAIN doesn't actually mutate).
/// `query_override` lets callers (currently `run_explain`) substitute
/// the SQL that's actually sent to the executor while keeping the
/// block's `params["query"]` text untouched.
///
/// `as_explain` re-routes the run as an EXPLAIN side-channel:
/// - skips the read-only gate (EXPLAIN is read-only by definition),
/// - skips the unscoped-destructive confirm (EXPLAIN doesn't mutate),
/// - skips the cache (EXPLAIN output is dialect-specific and small —
///   not worth poisoning the main cache slot),
/// - leaves `block.state` and `block.cached_result.results` untouched
///   so the block keeps showing whatever the last `r` produced,
/// - tags the spawn `RunningKind::Explain` so the result handler
///   merges into `cached_result["plan"]` and auto-switches to the
///   Plan tab.
pub(crate) fn run_db_block_inner(
    app: &mut App,
    segment_idx: usize,
    force_unscoped: bool,
    query_override: Option<String>,
    as_explain: bool,
) {
    let Some(doc) = app.document() else { return };
    // Snapshot the block so we can release the immutable doc borrow
    // before mutating later.
    let block = match doc.segments().get(segment_idx) {
        Some(Segment::Block(b)) => b.clone(),
        _ => return,
    };

    if !block.is_db() {
        app.set_status(
            StatusKind::Info,
            format!("`{}` blocks aren't runnable yet", block.block_type),
        );
        return;
    }

    // Build DbParams from the block's params blob. The fence parser
    // accepts both `connection` (info-string) and `connection_id`
    // (legacy JSON body); we accept either.
    let connection_id_raw = block
        .params
        .get("connection_id")
        .or_else(|| block.params.get("connection"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if connection_id_raw.is_empty() {
        app.set_status(
            StatusKind::Error,
            "no connection set on this block (add `connection=<id>` to the fence)",
        );
        return;
    }
    let raw_query = query_override.unwrap_or_else(|| {
        block
            .params
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string()
    });
    if raw_query.is_empty() {
        app.set_status(StatusKind::Error, "empty SQL");
        return;
    }
    // Pre-flight resolves env vars + block refs + connection name.
    // These are fast (in-memory + a couple of SQLite reads) so we
    // keep them on the dispatch thread; only the actual query goes
    // async. If any pre-flight step fails the run never spawns —
    // surface the error and bail.
    let env_vars: std::collections::HashMap<String, String> = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current()
            .block_on(load_active_env_vars(&app.environments_store))
    })
    .unwrap_or_default();
    let resolved = match app.document() {
        Some(d) => resolve_block_refs(d.segments(), segment_idx, &raw_query, &env_vars),
        None => Ok((raw_query.clone(), Vec::new())),
    };
    let (query, bind_values) = match resolved {
        Ok(qb) => qb,
        Err(msg) => {
            if let Some(doc) = app.tabs.active_document_mut() {
                if let Some(b) = doc.block_at_mut(segment_idx) {
                    b.state = ExecutionState::Error(msg.clone());
                    b.cached_result = None;
                }
            }
            app.set_status(StatusKind::Error, msg);
            return;
        }
    };
    let limit = block
        .params
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(100);
    // Per-query timeout opt-in via `timeout=` token in the fence
    // (parser writes `timeout_ms`). `None` → executor falls back to
    // the connection's default timeout, then to 30s if the
    // connection has no override either.
    let timeout_ms = block.params.get("timeout_ms").and_then(|v| v.as_u64());

    let store = app.connections_store.clone();
    let resolved = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current()
            .block_on(resolve_connection_id(&store, &connection_id_raw))
    });
    let connection_id = match resolved {
        Ok(id) => id,
        Err(msg) => {
            if let Some(doc) = app.tabs.active_document_mut() {
                if let Some(b) = doc.block_at_mut(segment_idx) {
                    b.state = ExecutionState::Error(msg.clone());
                    b.cached_result = None;
                }
            }
            app.set_status(StatusKind::Error, msg);
            return;
        }
    };

    // Read-only gate: when the connection is flagged `is_readonly`,
    // any write statement is blocked outright. There's no confirm
    // path here — the user has to either flip the conn's flag or
    // pick a different connection. Sync lookup via `block_in_place`,
    // matching the rest of `apply_run_block` (already on the
    // dispatch thread; small SQLite read). Skipped for EXPLAIN —
    // the wrapped query is a read by definition.
    if !as_explain {
        let store = app.connections_store.clone();
        let conn_meta = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(store.get(&connection_id))
        });
        let is_readonly_conn = matches!(
            &conn_meta,
            Ok(Some(c)) if c.is_readonly
        );
        if is_readonly_conn && is_writing_query(&raw_query) {
            let msg =
                "connection is read-only — flip the flag or pick a writable connection".to_string();
            if let Some(doc) = app.tabs.active_document_mut() {
                if let Some(b) = doc.block_at_mut(segment_idx) {
                    b.state = ExecutionState::Error(msg.clone());
                    b.cached_result = None;
                }
            }
            app.set_status(StatusKind::Error, msg);
            return;
        }
    }

    // Confirm gate: unscoped UPDATE/DELETE (no WHERE) is the kind
    // of slip that nukes a whole table. Pop a y/n modal so the user
    // explicitly OKs it. Skipped when `force_unscoped` is true (the
    // user already said yes from the previous popup) — and EXPLAIN
    // calls always pass force_unscoped, so this branch is also a
    // no-op for the side-channel.
    if !force_unscoped && is_unscoped_destructive(&raw_query) {
        let s = strip_leading_sql_comments(&raw_query);
        let kind: String = s
            .chars()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect::<String>()
            .to_ascii_uppercase();
        let reason = format!("{kind} without WHERE will affect every row");
        app.modal = Some(crate::modal::Modal::DbConfirmRun(
            crate::app::DbConfirmRunState {
                segment_idx,
                reason,
            },
        ));
        app.vim.mode = crate::vim::mode::Mode::Modal;
        return;
    }

    // Cache check — only for read queries; mutations always
    // re-execute. Pulls the active pane's file path (cache is
    // per-file) and computes the same hash recipe the desktop uses
    // (`raw_query` + only the env vars referenced in the body, then
    // `compute_block_hash`). Hit → set state to `Cached`, paint the
    // ⛁ summary, skip the spawn entirely. Miss → keep `cache_key`
    // so `handle_db_block_result` writes on success.
    //
    // EXPLAIN bypasses the cache entirely: the output is dialect-
    // specific, small, and stat-sensitive (the same EXPLAIN can
    // produce different plans across runs as the planner re-costs).
    // Caching it would either pollute the main query's cache slot
    // or need a separate slot, neither of which is worth shipping.
    let cache_key: Option<(String, String)> = if as_explain {
        None
    } else {
        let file_path: Option<String> = app
            .active_pane()
            .and_then(|p| p.document_path.as_ref())
            .map(|p| p.to_string_lossy().to_string());
        if is_cacheable_query(&raw_query) {
            file_path.as_deref().map(|fp| {
                let hash = compute_db_cache_hash(&raw_query, Some(&connection_id), &env_vars);
                (fp.to_string(), hash)
            })
        } else {
            None
        }
    };
    if let Some((fp, hash)) = cache_key.as_ref() {
        let app_pool = app.pool_manager.app_pool().clone();
        let fp_owned = fp.clone();
        let hash_owned = hash.clone();
        let cached =
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(
                    httui_core::block_results::get_block_result(&app_pool, &fp_owned, &hash_owned),
                )
            })
            .ok()
            .flatten();
        if let Some(row) = cached {
            if row.status == "success" {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&row.response) {
                    let summary = db_summary_from_value(Some(&value), row.elapsed_ms as u64);
                    if let Some(doc) = app.tabs.active_document_mut() {
                        if let Some(b) = doc.block_at_mut(segment_idx) {
                            b.state = ExecutionState::Cached;
                            b.cached_result = Some(value);
                        }
                    }
                    app.set_status(StatusKind::Info, format!("⛁ cached · {summary}"));
                    return;
                }
            }
        }
    }

    // Flip state to `Running` so the renderer paints the spinner /
    // yellow border. Skipped for EXPLAIN — the original query's
    // output stays visible while the plan loads in the background;
    // the status bar carries the "explaining…" affordance instead.
    if !as_explain {
        if let Some(doc) = app.tabs.active_document_mut() {
            if let Some(b) = doc.block_at_mut(segment_idx) {
                b.state = ExecutionState::Running;
            }
        }
    } else {
        app.set_status(StatusKind::Info, "explaining…");
    }

    let token = CancellationToken::new();
    let kind = if as_explain {
        crate::app::RunningKind::Explain
    } else {
        crate::app::RunningKind::Run
    };
    spawn_db_query(
        app,
        segment_idx,
        kind,
        token,
        connection_id,
        query,
        bind_values,
        limit,
        0,
        timeout_ms,
        cache_key,
    );
}

/// Common spawn path for both initial runs and load-more pages.
/// Captures the executor params, fires `tokio::spawn`, stores the
/// cancel handle on `App.running_query`, and arranges for the
/// completion `AppEvent::DbBlockResult` to land back in the main
/// loop. Caller is responsible for setting the block's state to
/// `ExecutionState::Running` before calling — this function only
/// owns the async dispatch.
#[allow(clippy::too_many_arguments)]
fn spawn_db_query(
    app: &mut App,
    segment_idx: usize,
    kind: crate::app::RunningKind,
    token: CancellationToken,
    connection_id: String,
    query: String,
    bind_values: Vec<serde_json::Value>,
    limit: u64,
    offset: u64,
    timeout_ms: Option<u64>,
    cache_key: Option<(String, String)>,
) {
    let Some(sender) = app.event_sender.clone() else {
        app.set_status(
            StatusKind::Error,
            "internal: no event sender wired (spawn aborted)",
        );
        return;
    };
    let executor = httui_core::executor::db::DbExecutor::new(app.pool_manager.clone());
    let params = build_db_executor_params(
        &connection_id,
        &query,
        &bind_values,
        offset,
        limit,
        timeout_ms,
    );
    let token_for_task = token.clone();
    let kind_for_task = kind;
    tokio::spawn(async move {
        let outcome = executor
            .execute_with_cancel(params, token_for_task)
            .await
            .map_err(|e| format!("{e}"));
        let result_kind = match kind_for_task {
            crate::app::RunningKind::Run => crate::event::DbBlockResultKind::Run,
            crate::app::RunningKind::LoadMore => crate::event::DbBlockResultKind::LoadMore,
            crate::app::RunningKind::Explain => crate::event::DbBlockResultKind::Explain,
        };
        let _ = sender.send(crate::event::AppEvent::DbBlockResult {
            segment_idx,
            kind: result_kind,
            outcome,
        });
    });
    app.running_query = Some(crate::app::RunningQuery {
        segment_idx,
        cancel: token,
        started_at: std::time::Instant::now(),
        kind,
        cache_key,
        bytes_received: 0,
    });
    // Anchor for `gr` (rerun). Only Run / Explain set this — LoadMore
    // is a transparent pagination follow-up, not a fresh user dispatch,
    // so we'd otherwise pin the anchor to a load-more idx that's a
    // no-op on rerun.
    if !matches!(kind, crate::app::RunningKind::LoadMore) {
        app.record_run_anchor(segment_idx);
    }
}

/// Fold the outcome of a backgrounded DB query (kicked off by
/// `apply_run_block` or the load-more prefetch) into the matching
/// block. Called by the main loop on `AppEvent::DbBlockResult`.
/// Always clears `app.running_query` so the next run / Ctrl-C
/// behave correctly.
pub fn handle_db_block_result(
    app: &mut App,
    segment_idx: usize,
    kind: crate::event::DbBlockResultKind,
    outcome: Result<httui_core::executor::db::types::DbResponse, String>,
) {
    // Cache key was stored on the running-query handle when
    // `apply_run_block` decided this was a cacheable read. Take it
    // *before* clearing the slot so the success branch below can
    // write back without re-deriving the hash.
    let cache_key = app.running_query.take().and_then(|q| q.cache_key);

    // Snapshot history-relevant metadata once, before we descend into
    // the per-kind match. Block fields (alias, query, connection)
    // don't change between Ok/Err and we need them in both branches
    // to record an entry. Only `DbBlockResultKind::Run` actually
    // emits a row — load-more / explain are not user "runs".
    let history_meta = if matches!(kind, crate::event::DbBlockResultKind::Run) {
        snapshot_db_history_meta(app, segment_idx)
    } else {
        None
    };

    use crate::event::DbBlockResultKind;
    use httui_core::executor::db::types::DbResult;
    match kind {
        DbBlockResultKind::Run => match outcome {
            Ok(response) => {
                let first_was_error =
                    matches!(response.results.first(), Some(DbResult::Error { .. }));
                let summary = summarize_db_response(&response);
                let value = serde_json::to_value(&response).ok();
                if let Some(doc) = app.tabs.active_document_mut() {
                    if let Some(b) = doc.block_at_mut(segment_idx) {
                        b.state = if first_was_error {
                            ExecutionState::Error(summary.clone())
                        } else {
                            ExecutionState::Success
                        };
                        b.cached_result = value.clone();
                    }
                }
                // Save to cache only on success — error responses
                // shouldn't poison subsequent runs (user fixes the
                // query and re-runs; we don't want to serve the old
                // error). Mirrors desktop behavior.
                if !first_was_error {
                    if let (Some((file_path, hash)), Some(value)) = (cache_key, value) {
                        save_db_cache_async(
                            app.pool_manager.app_pool().clone(),
                            file_path,
                            hash,
                            value,
                            response.stats.elapsed_ms,
                            &response.results,
                        );
                    }
                }
                // Record run history (metadata only — never the
                // result rows). Outcome distinguishes a SELECT/
                // mutation success from a per-statement error.
                if let Some(meta) = history_meta.as_ref() {
                    let elapsed = response.stats.elapsed_ms;
                    let (status, response_size) = derive_db_history_stats(&response);
                    let outcome_str = if first_was_error { "error" } else { "success" };
                    record_db_history_async(
                        app.pool_manager.app_pool().clone(),
                        meta.clone(),
                        Some(elapsed as i64),
                        status,
                        response_size,
                        outcome_str,
                    );
                }

                if first_was_error {
                    app.set_status(StatusKind::Error, summary);
                } else {
                    app.set_status(StatusKind::Info, summary);
                }
            }
            Err(msg) => {
                if let Some(doc) = app.tabs.active_document_mut() {
                    if let Some(b) = doc.block_at_mut(segment_idx) {
                        b.state = ExecutionState::Error(msg.clone());
                        b.cached_result = None;
                    }
                }
                if let Some(meta) = history_meta.as_ref() {
                    let outcome_str = if msg.to_lowercase().contains("cancel") {
                        "cancelled"
                    } else {
                        "error"
                    };
                    record_db_history_async(
                        app.pool_manager.app_pool().clone(),
                        meta.clone(),
                        None,
                        None,
                        None,
                        outcome_str,
                    );
                }
                app.set_status(StatusKind::Error, msg);
            }
        },
        DbBlockResultKind::LoadMore => match outcome {
            Ok(response) => {
                let (new_rows, new_has_more) = match response.results.first() {
                    Some(DbResult::Select { rows, has_more, .. }) => (rows.clone(), *has_more),
                    Some(DbResult::Error { message, .. }) => {
                        app.set_status(StatusKind::Error, format!("load more: {message}"));
                        return;
                    }
                    _ => {
                        app.set_status(StatusKind::Error, "load more: unexpected response shape");
                        return;
                    }
                };
                let new_total = if let Some(doc) = app.tabs.active_document_mut() {
                    if let Some(b) = doc.block_at_mut(segment_idx) {
                        if let Some(cached) = b.cached_result.as_mut() {
                            if let Some(first) = cached
                                .get_mut("results")
                                .and_then(|v| v.as_array_mut())
                                .and_then(|a| a.first_mut())
                            {
                                if let Some(rows) =
                                    first.get_mut("rows").and_then(|v| v.as_array_mut())
                                {
                                    rows.extend(new_rows);
                                    let total = rows.len();
                                    if let Some(slot) = first.get_mut("has_more") {
                                        *slot = serde_json::Value::Bool(new_has_more);
                                    }
                                    total
                                } else {
                                    0
                                }
                            } else {
                                0
                            }
                        } else {
                            0
                        }
                    } else {
                        0
                    }
                } else {
                    0
                };
                let suffix = if new_has_more { "+" } else { "" };
                app.set_status(StatusKind::Info, format!("loaded {new_total}{suffix} rows"));
            }
            Err(msg) => {
                app.set_status(StatusKind::Error, format!("load more: {msg}"));
            }
        },
        DbBlockResultKind::Explain => match outcome {
            Ok(response) => {
                // Stuff the EXPLAIN response under `cached_result.plan`
                // without touching `cached_result.results` — the
                // user's last `r` output stays visible. If the block
                // never ran a `r` (no cached_result yet), seed a
                // minimal envelope so the Plan tab has somewhere to
                // hang.
                let plan_value = serde_json::to_value(&response).ok();
                let first_was_error =
                    matches!(response.results.first(), Some(DbResult::Error { .. }));
                let block_id = app
                    .tabs
                    .active_document()
                    .and_then(|d| d.block_at(segment_idx))
                    .map(|b| b.id);
                if let Some(doc) = app.tabs.active_document_mut() {
                    if let Some(b) = doc.block_at_mut(segment_idx) {
                        let target = b.cached_result.get_or_insert_with(|| {
                            serde_json::json!({
                                "results": [],
                                "messages": [],
                                "stats": { "elapsed_ms": 0 }
                            })
                        });
                        if let Some(obj) = target.as_object_mut() {
                            obj.insert(
                                "plan".into(),
                                plan_value.unwrap_or(serde_json::Value::Null),
                            );
                        }
                    }
                }
                if first_was_error {
                    let msg = summarize_db_response(&response);
                    app.set_status(StatusKind::Error, format!("explain: {msg}"));
                } else {
                    // Auto-switch to the Plan tab so the user sees the
                    // result without an extra `gt`-cycle through tabs.
                    if let Some(id) = block_id {
                        app.set_result_tab(id, crate::app::ResultPanelTab::Plan);
                    }
                    app.set_status(
                        StatusKind::Info,
                        format!("plan · {}ms", response.stats.elapsed_ms),
                    );
                }
            }
            Err(msg) => {
                app.set_status(StatusKind::Error, format!("explain: {msg}"));
            }
        },
    }
}

/// Cancel an in-flight DB query, if any. Called from the
/// dispatcher when `Ctrl-C` arrives while `app.running_query` is
/// `Some`. The actual abort is reported back via the regular
/// `DbBlockResult` path (the executor's cancel-aware future
/// resolves to `Err("Request cancelled")`).
pub fn cancel_running_query(app: &mut App) -> bool {
    let Some(rq) = app.running_query.as_ref() else {
        return false;
    };
    rq.cancel.cancel();
    app.set_status(StatusKind::Info, "cancelling query…");
    true
}

/// Fire the next page of rows for a paginated DB block. Mirrors
/// `apply_run_block` but with `offset = rows.len()` and merge-on-
/// completion (the result handler appends instead of replacing the
/// `cached_result`). Returns `Ok(())` on dispatch, `Err(msg)` if
/// the pre-flight (no cache, no connection, ref resolution …)
/// failed — the caller surfaces that as a status hint.
pub(crate) fn load_more_db_block(app: &mut App, segment_idx: usize) -> Result<(), String> {
    if app.running_query.is_some() {
        return Err("another query is already running".into());
    }
    // Snapshot the block; release the immutable doc borrow before
    // any later mutation.
    let block = {
        let doc = app.document().ok_or_else(|| "no document".to_string())?;
        match doc.segments().get(segment_idx) {
            Some(Segment::Block(b)) => b.clone(),
            _ => return Err("block missing".into()),
        }
    };
    if !block.is_db() {
        return Err("not a DB block".into());
    }

    let cached = block
        .cached_result
        .as_ref()
        .ok_or_else(|| "no result cached yet".to_string())?;
    let first = cached
        .get("results")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .ok_or_else(|| "result has no rows".to_string())?;
    if first.get("kind").and_then(|v| v.as_str()) != Some("select") {
        return Err("not a select result".into());
    }
    let has_more = first
        .get("has_more")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !has_more {
        return Err("no more rows".into());
    }
    let current_offset = first
        .get("rows")
        .and_then(|v| v.as_array())
        .map(|a| a.len() as u64)
        .unwrap_or(0);

    let raw_query = block
        .params
        .get("query")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if raw_query.is_empty() {
        return Err("empty SQL".into());
    }
    let connection_id_raw = block
        .params
        .get("connection_id")
        .or_else(|| block.params.get("connection"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if connection_id_raw.is_empty() {
        return Err("no connection on block".into());
    }
    let limit = block
        .params
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(100);
    let timeout_ms = block.params.get("timeout_ms").and_then(|v| v.as_u64());

    let env_vars: std::collections::HashMap<String, String> = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current()
            .block_on(load_active_env_vars(&app.environments_store))
    })
    .unwrap_or_default();
    let (query, bind_values) = match app.document() {
        Some(d) => resolve_block_refs(d.segments(), segment_idx, &raw_query, &env_vars)?,
        None => (raw_query.clone(), Vec::new()),
    };
    let store = app.connections_store.clone();
    let connection_id = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current()
            .block_on(resolve_connection_id(&store, &connection_id_raw))
    })?;

    let token = CancellationToken::new();
    // Pagination doesn't write to the cache — the on-disk entry is
    // keyed by the original query+conn+env, not by `(query, offset)`.
    // Bumping cache on every load-more page would either bloat with
    // partial responses or, worse, overwrite the canonical entry
    // with a partial one.
    spawn_db_query(
        app,
        segment_idx,
        crate::app::RunningKind::LoadMore,
        token,
        connection_id,
        query,
        bind_values,
        limit,
        current_offset,
        timeout_ms,
        None,
    );
    Ok(())
}

// ─── Export picker ─────────────────────────────────────────────────

/// `gx` on a DB or HTTP block — open the export-format picker.
/// Block-type aware:
///   - DB: validates `cached_result.results[0]` is a SELECT with ≥1
///     row, picker shows CSV/JSON/Markdown/INSERT.
///   - HTTP: any HTTP block (no result needed — code-gen exports
///     the *request*), picker shows cURL/Fetch/Python/HTTPie/.http.
///
/// Failures surface as `Err(_)` with a status hint; success flips
/// the mode and stashes [`DbExportPickerState`] on `app`. Cursor is
/// allowed to move while the picker is open — confirm re-resolves
/// the block via the saved `segment_idx`.
pub fn open_export_picker(app: &mut App) -> Result<(), String> {
    let segment_idx = match app.document().map(|d| d.cursor()) {
        Some(Cursor::InBlock { segment_idx, .. })
        | Some(Cursor::InBlockResult { segment_idx, .. }) => segment_idx,
        _ => return Err("place the cursor on a block first".into()),
    };

    let block = match app
        .document()
        .and_then(|d| d.segments().get(segment_idx).cloned())
    {
        Some(Segment::Block(b)) => b,
        _ => return Err("place the cursor on a block first".into()),
    };

    let formats: &'static [crate::app::BlockExportFormat] = if block.is_db() {
        // DB: needs a SELECT result with rows — code-gen wouldn't
        // make sense for an empty / mutation result.
        let cache = block
            .cached_result
            .as_ref()
            .ok_or_else(|| "run the block before exporting its result".to_string())?;
        let first = cache
            .get("results")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .ok_or_else(|| "no result to export — run the block first".to_string())?;
        let kind = first.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        if kind != "select" {
            return Err(format!(
                "{kind} results have no tabular form — export is for SELECT only"
            ));
        }
        let row_count = first
            .get("rows")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        if row_count == 0 {
            return Err("result has no rows to export".into());
        }
        crate::app::BlockExportFormat::DB_FORMATS
    } else if block.is_http() {
        // HTTP: code-gen exports the *request* (method+url+headers+
        // body), so we only need a non-empty URL. Run state isn't
        // required.
        let url_ok = block
            .params
            .get("url")
            .and_then(|v| v.as_str())
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        if !url_ok {
            return Err("set a URL on the block before exporting".into());
        }
        crate::app::BlockExportFormat::HTTP_FORMATS
    } else {
        return Err(format!(
            "`{}` blocks don't support export yet",
            block.block_type
        ));
    };

    app.modal = Some(crate::modal::Modal::DbExportPicker(
        crate::app::DbExportPickerState::new(segment_idx, formats),
    ));
    app.vim.mode = crate::vim::mode::Mode::Modal;
    app.vim.reset_pending();
    Ok(())
}

pub fn close_export_picker(app: &mut App) {
    if matches!(app.modal, Some(crate::modal::Modal::DbExportPicker(_))) {
        app.modal = None;
    }
    app.vim.enter_normal();
}

pub fn move_export_picker_cursor(app: &mut App, delta: i32) {
    let Some(crate::modal::Modal::DbExportPicker(state)) = app.modal.as_mut() else {
        return;
    };
    let n = state.formats.len() as i64;
    if n == 0 {
        return;
    }
    // Wrap so j/k cycle the list — feels right for a 4-5-item list
    // where the user is likely to overshoot.
    let next = ((state.selected as i64) + delta as i64).rem_euclid(n);
    state.selected = next as usize;
}

/// `Enter` in the picker — dispatch to the right serializer based
/// on the block type and copy the output to the clipboard. The
/// popup closes either way; clipboard failure shows in the status
/// line so the user notices it didn't paste.
pub fn confirm_export_picker(app: &mut App) {
    let state = match app.modal.take() {
        Some(crate::modal::Modal::DbExportPicker(s)) => s,
        other => {
            app.modal = other;
            app.vim.enter_normal();
            return;
        }
    };
    app.vim.enter_normal();

    let format = match state.formats.get(state.selected).copied() {
        Some(f) => f,
        None => return,
    };

    let block = match app
        .document()
        .and_then(|d| d.segments().get(state.segment_idx).cloned())
    {
        Some(Segment::Block(b)) => b,
        _ => {
            app.set_status(StatusKind::Error, "block disappeared from the document");
            return;
        }
    };

    let payload_with_summary = match format {
        // ─── DB formats ───
        crate::app::BlockExportFormat::Csv
        | crate::app::BlockExportFormat::Json
        | crate::app::BlockExportFormat::Markdown
        | crate::app::BlockExportFormat::Insert => {
            let cache = match block.cached_result.as_ref() {
                Some(v) => v,
                None => {
                    app.set_status(StatusKind::Info, "block has no cached result");
                    return;
                }
            };
            let first = match cache
                .get("results")
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
            {
                Some(v) => v,
                None => {
                    app.set_status(StatusKind::Info, "result is empty");
                    return;
                }
            };
            let columns: Vec<httui_core::db::connections::ColumnInfo> = first
                .get("columns")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let rows: Vec<serde_json::Value> = first
                .get("rows")
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default();
            if !httui_core::blocks::db_export::has_exportable_rows(&columns, &rows) {
                app.set_status(StatusKind::Info, "no rows to export");
                return;
            }
            let payload = match format {
                crate::app::BlockExportFormat::Csv => {
                    httui_core::blocks::db_export::to_csv(&columns, &rows)
                }
                crate::app::BlockExportFormat::Json => {
                    httui_core::blocks::db_export::to_json(&rows)
                }
                crate::app::BlockExportFormat::Markdown => {
                    httui_core::blocks::db_export::to_markdown(&columns, &rows)
                }
                crate::app::BlockExportFormat::Insert => {
                    let sql = block
                        .params
                        .get("query")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let table =
                        httui_core::blocks::db_export::infer_table_name(sql).unwrap_or_default();
                    httui_core::blocks::db_export::to_inserts(&columns, &rows, &table)
                }
                _ => unreachable!("filtered by outer match"),
            };
            let summary = format!(
                "copied {} ({} rows, {} bytes) to clipboard",
                format.label(),
                rows.len(),
                payload.len()
            );
            (payload, summary)
        }

        // ─── HTTP formats ───
        crate::app::BlockExportFormat::Curl
        | crate::app::BlockExportFormat::Fetch
        | crate::app::BlockExportFormat::Python
        | crate::app::BlockExportFormat::HTTPie
        | crate::app::BlockExportFormat::HttpFile => {
            // Resolve `{{refs}}` (env vars + block deps) BEFORE
            // serializing — the user expects a snippet they can
            // paste as-is. Carrying placeholders into the output
            // means the cURL command fails when run.
            let env_vars = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current()
                    .block_on(load_active_env_vars(&app.environments_store))
            })
            .unwrap_or_default();
            let segments_snapshot: Vec<Segment> = app
                .document()
                .map(|d| d.segments().to_vec())
                .unwrap_or_default();
            let mut resolved = block.params.clone();
            if let Err(msg) = crate::commands::http::resolve_in_http_params(
                &mut resolved,
                &segments_snapshot,
                state.segment_idx,
                &env_vars,
            ) {
                app.set_status(StatusKind::Error, format!("ref resolution failed: {msg}"));
                return;
            }
            let payload = match format {
                crate::app::BlockExportFormat::Curl => {
                    httui_core::blocks::http_codegen::to_curl(&resolved)
                }
                crate::app::BlockExportFormat::Fetch => {
                    httui_core::blocks::http_codegen::to_fetch(&resolved)
                }
                crate::app::BlockExportFormat::Python => {
                    httui_core::blocks::http_codegen::to_python(&resolved)
                }
                crate::app::BlockExportFormat::HTTPie => {
                    httui_core::blocks::http_codegen::to_httpie(&resolved)
                }
                crate::app::BlockExportFormat::HttpFile => {
                    httui_core::blocks::http_codegen::to_http_file(&resolved)
                }
                _ => unreachable!("filtered by outer match"),
            };
            let summary = format!(
                "copied {} ({} bytes) to clipboard",
                format.label(),
                payload.len()
            );
            (payload, summary)
        }
    };

    let (payload, summary) = payload_with_summary;
    match crate::clipboard::set_text(&payload) {
        Ok(()) => {
            app.set_status(StatusKind::Info, summary);
        }
        Err(e) => {
            app.set_status(StatusKind::Error, e);
        }
    }
}

// ─── DB block run history ────────────────────────────────────────

/// Snapshot of the bits we need to write a `block_run_history` row
/// for a DB block run. Captured up-front (before the result handler
/// mutates state) so the Ok and Err arms can share a single insert
/// path without re-walking the segment list.
#[derive(Clone)]
pub struct DbHistoryMeta {
    pub file_path: String,
    pub block_alias: String,
    /// Stored in the `method` column as `db:<driver>` (e.g.
    /// `db:postgres`). Mirrors how the HTTP path uses `method` as
    /// the request "kind" — same column, different namespace.
    pub method: String,
    /// SQL preview (~200 chars, single-line). Goes in the
    /// `url_canonical` column whose semantic role is "what was
    /// run" — for HTTP that's URL+query; for DB it's the SQL.
    pub url_canonical: String,
    pub request_size: i64,
}

pub fn snapshot_db_history_meta(app: &App, segment_idx: usize) -> Option<DbHistoryMeta> {
    let tab = app.tabs.tabs.get(app.tabs.active())?;
    let file_path = tab.active_leaf().document_path.as_ref()?;
    let file_path = file_path.to_string_lossy().to_string();
    let doc = app.document()?;
    let block = match doc.segments().get(segment_idx)? {
        Segment::Block(b) => b,
        _ => return None,
    };
    if !block.is_db() {
        return None;
    }
    let alias = block
        .alias
        .as_deref()
        .filter(|s| !s.is_empty())?
        .to_string();
    // `block_type` is `db-postgres`, `db-mysql`, `db-sqlite`. Strip
    // the `db-` prefix for the namespaced method label so the
    // history modal shows a clean `postgres` / `mysql` / `sqlite`
    // chip instead of repeating `db-` everywhere.
    let driver = block
        .block_type
        .strip_prefix("db-")
        .unwrap_or(&block.block_type);
    let method = format!("db:{driver}");

    let query = block
        .params
        .get("query")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let url_canonical = preview_sql(&query);
    let request_size = query.len() as i64;
    Some(DbHistoryMeta {
        file_path,
        block_alias: alias,
        method,
        url_canonical,
        request_size,
    })
}

/// Collapse newlines to spaces and trim to ~200 chars so the
/// history modal's row stays readable. Suffix `…` when truncated.
fn preview_sql(sql: &str) -> String {
    const MAX: usize = 200;
    let collapsed = sql
        .replace(['\r', '\n', '\t'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if collapsed.chars().count() > MAX {
        let truncated: String = collapsed.chars().take(MAX).collect();
        format!("{truncated}…")
    } else {
        collapsed
    }
}

/// Derive the `(status, response_size)` columns from a successful
/// DB response. Status borrows the column for "result kind size":
/// `rows.len()` for a SELECT, `rows_affected` for a mutation,
/// `None` for an error result. Response size is the serialized
/// JSON length — coarse but useful for spotting regressions.
pub fn derive_db_history_stats(
    response: &httui_core::executor::db::types::DbResponse,
) -> (Option<i64>, Option<i64>) {
    use httui_core::executor::db::types::DbResult;
    let status = match response.results.first() {
        Some(DbResult::Select { rows, .. }) => Some(rows.len() as i64),
        Some(DbResult::Mutation { rows_affected }) => Some(*rows_affected as i64),
        _ => None,
    };
    let response_size = serde_json::to_string(response).ok().map(|s| s.len() as i64);
    (status, response_size)
}

/// Spawn the SQLite insert in the background (mirrors the HTTP
/// path). Failures land in the tracing log — history is best-
/// effort and never blocks the user.
pub fn record_db_history_async(
    pool: sqlx::SqlitePool,
    meta: DbHistoryMeta,
    elapsed_ms: Option<i64>,
    status: Option<i64>,
    response_size: Option<i64>,
    outcome: &'static str,
) {
    let entry = httui_core::block_history::InsertEntry {
        file_path: meta.file_path,
        block_alias: meta.block_alias,
        method: meta.method,
        url_canonical: meta.url_canonical,
        status,
        request_size: Some(meta.request_size),
        response_size,
        elapsed_ms,
        outcome: outcome.to_string(),
        plan: None,
    };
    tokio::spawn(async move {
        if let Err(e) = httui_core::block_history::insert_history_entry(&pool, entry).await {
            tracing::warn!("db block history insert failed: {e}");
        }
    });
}

// ─── Settings modal (/3) ────────────────────────────────────────────

/// `gs` on a DB or HTTP block — open the settings modal prefilled
/// with the current values. Block-type aware:
///   - DB: limit + timeout fields, focus starts on Limit
///   - HTTP: timeout only (no row-cap concept), focus on Timeout
///
/// Tab cycles fields (no-op on single-field HTTP modal), Enter saves
/// all, Esc cancels. Non-DB / non-HTTP blocks surface a status hint
/// and bail.
pub fn open_db_settings_modal(app: &mut App) -> Result<(), String> {
    let segment_idx = match app.document().map(|d| d.cursor()) {
        Some(Cursor::InBlock { segment_idx, .. })
        | Some(Cursor::InBlockResult { segment_idx, .. }) => segment_idx,
        _ => return Err("place the cursor on a block first".into()),
    };
    let block = match app
        .document()
        .and_then(|d| d.segments().get(segment_idx).cloned())
    {
        Some(crate::buffer::Segment::Block(b)) => b,
        _ => return Err("place the cursor on a block first".into()),
    };
    if !block.is_db() && !block.is_http() {
        return Err(format!(
            "`{}` blocks have no settings yet",
            block.block_type
        ));
    }

    // Build the field list per block type. Order pinned here —
    // Tab/BackTab cycle in this order; users build muscle memory.
    // Future fields slot in by adding a new entry here, no other
    // code change. All values are stringified u64 (positive
    // integer) — empty input means "clear the field".
    let mut fields: Vec<crate::app::SettingsField> = Vec::new();
    if block.is_db() {
        let limit_str = block
            .params
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|n| n.to_string())
            .unwrap_or_default();
        fields.push(crate::app::SettingsField {
            label: "Limit (rows, blank = no cap)",
            key: "limit",
            input: crate::vim::lineedit::LineEdit::from_str(limit_str),
        });
    }
    let timeout_str = block
        .params
        .get("timeout_ms")
        .and_then(|v| v.as_u64())
        .map(|n| n.to_string())
        .unwrap_or_default();
    fields.push(crate::app::SettingsField {
        label: "Timeout (ms, blank = default)",
        key: "timeout_ms",
        input: crate::vim::lineedit::LineEdit::from_str(timeout_str),
    });

    app.modal = Some(crate::modal::Modal::DbSettings(crate::app::DbSettingsState {
        segment_idx,
        fields,
        focus: 0,
    }));
    app.vim.mode = crate::vim::mode::Mode::DbSettings;
    app.vim.reset_pending();
    Ok(())
}

pub fn close_db_settings_modal(app: &mut App) {
    if matches!(app.modal, Some(crate::modal::Modal::DbSettings(_))) {
        app.modal = None;
    }
    app.vim.enter_normal();
}

pub fn db_settings_focus_step(app: &mut App, delta: i32) {
    let Some(state) = app.db_settings_mut() else {
        return;
    };
    if delta >= 0 {
        state.focus_next();
    } else {
        state.focus_prev();
    }
}

/// `<CR>` in the modal — validate every field's input and write
/// back to `block.params`. Empty input clears the matching key;
/// non-numeric / out-of-range input keeps the modal open and
/// surfaces a per-field status error so the user can fix without
/// losing the other inputs. The fields are walked in vector order
/// — Tab order — and validation short-circuits on the first
/// failure (label included in the error so the user knows which
/// field needs attention).
pub fn confirm_db_settings_modal(app: &mut App) {
    let Some(state) = app.db_settings() else {
        app.vim.enter_normal();
        return;
    };
    let segment_idx = state.segment_idx;

    // Validate every field. Each field is `(key, parsed_value)` —
    // the writeback step inserts when `Some`, removes when `None`.
    let mut writes: Vec<(&'static str, Option<u64>)> = Vec::with_capacity(state.fields.len());
    for field in &state.fields {
        let raw = field.input.as_str().trim().to_string();
        match parse_optional_u64(&raw) {
            Ok(v) => writes.push((field.key, v)),
            Err(e) => {
                // Use the field label (already user-friendly) as the
                // prefix so errors are unambiguous in multi-field
                // modals.
                let label_short = field
                    .label
                    .split_whitespace()
                    .next()
                    .unwrap_or(field.key)
                    .to_lowercase();
                app.set_status(StatusKind::Error, format!("{label_short}: {e}"));
                return;
            }
        }
    }

    // All inputs validated — close the modal and persist.
    if matches!(app.modal, Some(crate::modal::Modal::DbSettings(_))) {
        app.modal = None;
    }
    app.vim.enter_normal();

    if let Some(doc) = app.tabs.active_document_mut() {
        doc.snapshot();
        if let Some(block) = doc.block_at_mut(segment_idx) {
            if let Some(obj) = block.params.as_object_mut() {
                for (key, value) in &writes {
                    match value {
                        Some(n) => {
                            obj.insert((*key).to_string(), serde_json::Value::Number((*n).into()));
                        }
                        None => {
                            obj.remove(*key);
                        }
                    }
                }
            }
        }
    }

    // Status summary — one chunk per field, comma-joined.
    let chunks: Vec<String> = writes
        .iter()
        .map(|(key, value)| match value {
            Some(n) => format!("{key} {n}"),
            None => format!("{key} cleared"),
        })
        .collect();
    let summary = if chunks.is_empty() {
        "settings unchanged".to_string()
    } else {
        format!("settings saved · {}", chunks.join(" · "))
    };
    app.set_status(StatusKind::Info, summary);
}

/// Parse a possibly-empty trimmed string as a `u64`. Empty input
/// returns `Ok(None)` so the caller can clear the field; non-empty
/// must be a valid `u64` (no negatives, no decimals).
fn parse_optional_u64(s: &str) -> Result<Option<u64>, String> {
    if s.is_empty() {
        return Ok(None);
    }
    s.parse::<u64>()
        .map(Some)
        .map_err(|_| format!("`{s}` is not a non-negative integer"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Document;

    // ───────────── doc / cache fixtures ─────────────

    fn make_doc(md: &str) -> Document {
        Document::from_markdown(md).expect("valid markdown")
    }

    fn set_cache(doc: &mut Document, idx: usize, v: serde_json::Value) {
        let block = doc
            .block_at_mut(idx)
            .expect("segment idx should be a block");
        block.cached_result = Some(v);
    }

    fn block_indices(doc: &Document) -> Vec<usize> {
        doc.segments()
            .iter()
            .enumerate()
            .filter_map(|(i, s)| matches!(s, Segment::Block(_)).then_some(i))
            .collect()
    }

    fn empty_env() -> std::collections::HashMap<String, String> {
        std::collections::HashMap::new()
    }

    fn env_map(pairs: &[(&str, &str)]) -> std::collections::HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn db_response(results: serde_json::Value) -> serde_json::Value {
        // Build a minimal `DbResponse`-shaped JSON. Pre-redesign caches
        // (no `results` array) bypass the shim — see `is_db_response_shape`.
        serde_json::json!({
            "results": results,
            "messages": [],
            "plan": serde_json::Value::Null,
            "stats": { "elapsed_ms": 12 }
        })
    }

    fn select_result(rows: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "kind": "select",
            "columns": [],
            "rows": rows,
            "has_more": false
        })
    }

    // ───────────── SQL classifiers ─────────────

    #[test]
    fn cacheable_query_recognizes_select_family() {
        // The classic safe-to-cache statements. SHOW/PRAGMA are
        // read-only too even though they don't return rows the
        // typical way — desktop caches them, we match.
        for q in &[
            "SELECT 1",
            "select 1",
            "  SELECT * FROM foo",
            "WITH x AS (...) SELECT 1",
            "EXPLAIN SELECT 1",
            "PRAGMA table_info('users')",
            "SHOW TABLES",
            "DESC users",
        ] {
            assert!(is_cacheable_query(q), "expected cacheable: {q}");
        }
    }

    #[test]
    fn cacheable_query_rejects_mutations() {
        // Anything that writes — never serve from cache.
        for q in &[
            "UPDATE users SET x = 1",
            "DELETE FROM users",
            "INSERT INTO users VALUES (1)",
            "REPLACE INTO users VALUES (1)",
            "CREATE TABLE x (id INT)",
            "DROP TABLE x",
            "ALTER TABLE x ADD COLUMN y INT",
            "TRUNCATE TABLE x",
        ] {
            assert!(!is_cacheable_query(q), "expected mutation: {q}");
        }
    }

    #[test]
    fn cacheable_query_strips_leading_comments() {
        // A header comment shouldn't fool the classifier — desktop
        // notes commonly start with a `-- description` line above
        // the SELECT.
        assert!(is_cacheable_query("-- daily report\nSELECT 1"));
        assert!(is_cacheable_query("/* multi\n   line */\nSELECT 1"));
        // Mutation behind a comment is still a mutation.
        assert!(!is_cacheable_query("-- cleanup job\nDELETE FROM users"));
    }

    #[test]
    fn writing_query_recognizes_mutations() {
        // The set the read-only gate refuses on RO connections.
        for q in &[
            "UPDATE users SET x=1",
            "DELETE FROM users",
            "INSERT INTO t VALUES (1)",
            "REPLACE INTO t VALUES (1)",
            "MERGE INTO t USING ...",
            "CREATE TABLE x (id INT)",
            "DROP TABLE x",
            "ALTER TABLE x ADD COLUMN y INT",
            "TRUNCATE TABLE x",
            "GRANT SELECT ON t TO u",
            "REVOKE SELECT ON t FROM u",
            "VACUUM",
        ] {
            assert!(is_writing_query(q), "expected write: {q}");
        }
    }

    #[test]
    fn writing_query_rejects_reads() {
        // Reads (and pseudo-reads) should NEVER be classified as
        // writing — otherwise SELECT against a RO conn would be
        // wrongly blocked.
        for q in &[
            "SELECT 1",
            "SELECT * FROM users",
            "WITH x AS (SELECT 1) SELECT * FROM x",
            "EXPLAIN SELECT 1",
            "PRAGMA table_info('x')",
            "SHOW TABLES",
            "DESC users",
        ] {
            assert!(!is_writing_query(q), "should not be write: {q}");
        }
    }

    #[test]
    fn unscoped_destructive_flags_update_without_where() {
        // Bare UPDATE / DELETE — the slip we want to confirm.
        assert!(is_unscoped_destructive("UPDATE users SET x = 1"));
        assert!(is_unscoped_destructive("DELETE FROM users"));
        assert!(is_unscoped_destructive("update users set name = 'x'"));
    }

    #[test]
    fn unscoped_destructive_passes_when_where_present() {
        // The whole point of the gate is to *not* prompt when
        // there's a WHERE clause — power users hit `r` constantly
        // and confirming every UPDATE would be obnoxious.
        assert!(!is_unscoped_destructive(
            "UPDATE users SET x = 1 WHERE id = 7"
        ));
        assert!(!is_unscoped_destructive(
            "DELETE FROM users WHERE active = 0"
        ));
        // Case-insensitive WHERE detection.
        assert!(!is_unscoped_destructive("delete from users where id < 10"));
    }

    #[test]
    fn unscoped_destructive_is_word_boundary_aware() {
        // A column literally named `whereabouts` shouldn't be
        // mistaken for the WHERE keyword. Same goes for any
        // identifier that contains the substring `where`.
        assert!(is_unscoped_destructive(
            "UPDATE users SET whereabouts = 'home'"
        ));
    }

    #[test]
    fn unscoped_destructive_skips_other_writes() {
        // CREATE / INSERT / DROP aren't part of the confirm gate —
        // they're either non-destructive (CREATE) or always
        // intentional (DROP TABLE has its own context).
        assert!(!is_unscoped_destructive("INSERT INTO users VALUES (1)"));
        assert!(!is_unscoped_destructive("DROP TABLE users"));
        assert!(!is_unscoped_destructive("CREATE TABLE t (id INT)"));
    }

    #[test]
    fn unscoped_destructive_strips_leading_comments() {
        // A `-- backup first` header above the DELETE shouldn't
        // hide the destructive intent from the gate.
        assert!(is_unscoped_destructive(
            "-- run after midnight\nDELETE FROM users"
        ));
        assert!(!is_unscoped_destructive(
            "-- legit\nDELETE FROM users WHERE inactive = 1"
        ));
    }

    // ───────────── cache hash ─────────────

    #[test]
    fn cache_hash_is_deterministic_for_same_inputs() {
        // Identical body / conn / env → identical hash. The cache
        // contract relies on this being stable across processes.
        let env = env_map(&[("TOKEN", "abc")]);
        let h1 = compute_db_cache_hash("SELECT 1 WHERE x = {{TOKEN}}", Some("conn-1"), &env);
        let h2 = compute_db_cache_hash("SELECT 1 WHERE x = {{TOKEN}}", Some("conn-1"), &env);
        assert_eq!(h1, h2);
    }

    #[test]
    fn cache_hash_changes_when_referenced_env_value_changes() {
        // The recipe folds in only env vars referenced via
        // `{{KEY}}`. Bump the value of a referenced var and the
        // hash must shift so the next run sees a miss.
        let body = "SELECT 1 WHERE x = {{TOKEN}}";
        let h_old = compute_db_cache_hash(body, Some("conn-1"), &env_map(&[("TOKEN", "old")]));
        let h_new = compute_db_cache_hash(body, Some("conn-1"), &env_map(&[("TOKEN", "new")]));
        assert_ne!(h_old, h_new);
    }

    #[test]
    fn cache_hash_ignores_unreferenced_env_vars() {
        // Env vars NOT referenced in the body don't affect the
        // hash — same desktop guarantee. A query that doesn't read
        // `{{X}}` should hit cache regardless of `X`'s current
        // value, so users switching environments don't pay for
        // every cached query.
        let body = "SELECT 1";
        let h1 = compute_db_cache_hash(body, Some("conn-1"), &env_map(&[]));
        let h2 = compute_db_cache_hash(body, Some("conn-1"), &env_map(&[("UNRELATED", "v")]));
        assert_eq!(h1, h2);
    }

    #[test]
    fn cache_hash_changes_with_connection_id() {
        // Same body against two connections must hash differently
        // — they could be pointing at different schemas.
        let body = "SELECT 1";
        let env = env_map(&[]);
        let h1 = compute_db_cache_hash(body, Some("conn-a"), &env);
        let h2 = compute_db_cache_hash(body, Some("conn-b"), &env);
        assert_ne!(h1, h2);
    }

    // ───────────── db_summary_from_value ─────────────

    #[test]
    fn db_summary_from_value_handles_select_with_extras() {
        // Multi-statement select → the summary describes results[0]
        // and appends `(+N more)`. Same wording the renderer uses.
        let value = serde_json::json!({
            "results": [
                { "kind": "select", "rows": [{}, {}, {}], "has_more": false },
                { "kind": "select", "rows": [{}], "has_more": false },
            ],
            "stats": { "elapsed_ms": 0 }
        });
        let s = db_summary_from_value(Some(&value), 12);
        assert_eq!(s, "3 rows · 12ms (+1 more)");
    }

    #[test]
    fn db_summary_from_value_describes_mutation() {
        let value = serde_json::json!({
            "results": [{ "kind": "mutation", "rows_affected": 7 }],
            "stats": { "elapsed_ms": 0 }
        });
        let s = db_summary_from_value(Some(&value), 4);
        assert_eq!(s, "7 affected · 4ms");
    }

    #[test]
    fn db_summary_from_value_appends_line_column_for_error() {
        // Postgres typically returns `position` byte offset which the
        // executor enriches into `(line, column)`. The summary should
        // surface that so users see *where* the parser tripped, not
        // just the message.
        let value = serde_json::json!({
            "results": [
                {
                    "kind": "error",
                    "message": "syntax error at or near \"FORM\"",
                    "line": 2,
                    "column": 5
                }
            ],
            "stats": { "elapsed_ms": 4 }
        });
        let s = db_summary_from_value(Some(&value), 4);
        assert_eq!(s, "error: syntax error at or near \"FORM\" at 2:5");
    }

    #[test]
    fn db_summary_from_value_omits_position_when_absent() {
        // Errors without positional info (older / generic driver
        // failures, MySQL parse errors that don't expose location)
        // should still render cleanly — just the message, no
        // dangling `at :`.
        let value = serde_json::json!({
            "results": [
                {
                    "kind": "error",
                    "message": "connection lost"
                }
            ],
            "stats": { "elapsed_ms": 0 }
        });
        let s = db_summary_from_value(Some(&value), 0);
        assert_eq!(s, "error: connection lost");
    }

    // ───────────── resolve_block_refs (bind-params) ─────────────
    //
    // These tests guard the security invariant: every `{{ref}}` value,
    // no matter what the upstream block emits, must leave the function
    // as a *bind value* — never as part of the SQL string. A malicious
    // value like `'; DROP TABLE x;` should land in the bind array
    // intact and reach the driver as a single string parameter.

    #[test]
    fn resolve_block_refs_replaces_refs_with_question_marks() {
        // Two-block doc; second block references the first by alias.
        // The output SQL must carry placeholders, never the raw value.
        let md =
            "```http alias=upstream\nGET /users/7\n```\n\n```db-postgres alias=q\nSELECT 1\n```\n";
        let mut doc = make_doc(md);
        let blocks = block_indices(&doc);
        set_cache(&mut doc, blocks[0], serde_json::json!({ "id": 7 }));
        let (sql, binds) = resolve_block_refs(
            doc.segments(),
            blocks[1],
            "SELECT * FROM users WHERE id = {{upstream.id}}",
            &empty_env(),
        )
        .expect("resolves");
        assert_eq!(sql, "SELECT * FROM users WHERE id = ?");
        assert_eq!(binds, vec![serde_json::json!(7)]);
    }

    #[test]
    fn resolve_block_refs_blocks_sql_injection_via_string_value() {
        // Classic injection payload returned by an upstream block: the
        // single-quote-and-DROP must NOT escape into the SQL string.
        // It belongs in the bind array as a single literal.
        let md = "```http alias=evil\nGET /\n```\n\n```db-postgres alias=q\nSELECT 1\n```\n";
        let mut doc = make_doc(md);
        let blocks = block_indices(&doc);
        let payload = "7'; DROP TABLE users; --";
        set_cache(&mut doc, blocks[0], serde_json::json!({ "id": payload }));
        let (sql, binds) = resolve_block_refs(
            doc.segments(),
            blocks[1],
            "SELECT * FROM users WHERE id = {{evil.id}}",
            &empty_env(),
        )
        .expect("resolves");
        assert_eq!(sql, "SELECT * FROM users WHERE id = ?");
        assert!(
            !sql.contains("DROP"),
            "injection payload leaked into SQL: {sql}"
        );
        assert_eq!(binds, vec![serde_json::Value::String(payload.to_string())]);
    }

    #[test]
    fn resolve_block_refs_emits_one_bind_per_placeholder_in_order() {
        // Multiple placeholders → array order matches placeholder order.
        // sqlx slices binds per-statement by `count_placeholders`, so
        // ordering matters when 04.2 multi-statement lands.
        let md = "```http alias=src\nGET /\n```\n\n```db-postgres alias=q\nSELECT 1\n```\n";
        let mut doc = make_doc(md);
        let blocks = block_indices(&doc);
        set_cache(
            &mut doc,
            blocks[0],
            serde_json::json!({ "a": 1, "b": "two", "c": true }),
        );
        let (sql, binds) = resolve_block_refs(
            doc.segments(),
            blocks[1],
            "SELECT {{src.a}}, {{src.b}}, {{src.c}}",
            &empty_env(),
        )
        .expect("resolves");
        assert_eq!(sql, "SELECT ?, ?, ?");
        assert_eq!(
            binds,
            vec![
                serde_json::json!(1),
                serde_json::json!("two"),
                serde_json::json!(true),
            ]
        );
    }

    #[test]
    fn resolve_block_refs_preserves_value_types() {
        // Number stays a Number (driver decides numeric coercion);
        // bool stays a Bool; null stays Null. Earlier code stringified
        // each into a SQL literal — that's what we're moving away from.
        let md = "```http alias=src\nGET /\n```\n\n```db-postgres alias=q\nSELECT 1\n```\n";
        let mut doc = make_doc(md);
        let blocks = block_indices(&doc);
        set_cache(
            &mut doc,
            blocks[0],
            serde_json::json!({ "n": 42, "f": false, "z": serde_json::Value::Null }),
        );
        let (_, binds) = resolve_block_refs(
            doc.segments(),
            blocks[1],
            "SELECT {{src.n}}, {{src.f}}, {{src.z}}",
            &empty_env(),
        )
        .expect("resolves");
        assert!(binds[0].is_number(), "number type lost: {:?}", binds[0]);
        assert!(binds[1].is_boolean(), "bool type lost: {:?}", binds[1]);
        assert!(binds[2].is_null(), "null type lost: {:?}", binds[2]);
    }

    #[test]
    fn resolve_block_refs_env_var_becomes_string_bind() {
        // Single-segment refs that don't match a block fall back to
        // env vars and bind as a String. This replaces the old path
        // that wrapped values in `'...'` SQL literals.
        let mut env = std::collections::HashMap::new();
        env.insert("API_TOKEN".to_string(), "abc-123".to_string());
        let md = "```db-postgres alias=q\nSELECT 1\n```\n";
        let doc = make_doc(md);
        let blocks = block_indices(&doc);
        let (sql, binds) =
            resolve_block_refs(doc.segments(), blocks[0], "SELECT {{API_TOKEN}}", &env)
                .expect("resolves");
        assert_eq!(sql, "SELECT ?");
        assert_eq!(binds, vec![serde_json::json!("abc-123")]);
    }

    #[test]
    fn resolve_block_refs_rejects_array_or_object_value() {
        // Driver-side bind can't take a JSON array or object on the
        // dialects we target — caller sees a clear error instead of a
        // silent stringify. Mirrors desktop behavior.
        let md = "```http alias=src\nGET /\n```\n\n```db-postgres alias=q\nSELECT 1\n```\n";
        let mut doc = make_doc(md);
        let blocks = block_indices(&doc);
        set_cache(
            &mut doc,
            blocks[0],
            serde_json::json!({ "items": [1, 2, 3] }),
        );
        let err = resolve_block_refs(
            doc.segments(),
            blocks[1],
            "SELECT * FROM x WHERE y = {{src.items}}",
            &empty_env(),
        )
        .expect_err("array values can't bind");
        assert!(err.contains("non-scalar"), "got: {err}");
    }

    #[test]
    fn resolve_block_refs_unknown_alias_errors() {
        // A dotted ref to a non-existent block fails loudly instead of
        // silently leaving the placeholder — same desktop semantics.
        let md = "```db-postgres alias=q\nSELECT 1\n```\n";
        let doc = make_doc(md);
        let blocks = block_indices(&doc);
        let err = resolve_block_refs(
            doc.segments(),
            blocks[0],
            "SELECT * FROM x WHERE y = {{ghost.id}}",
            &empty_env(),
        )
        .expect_err("ghost alias has no upstream block");
        assert!(err.contains("ghost"), "got: {err}");
    }

    #[test]
    fn resolve_block_refs_preserves_query_when_no_refs_present() {
        // Plain SQL passes through verbatim with an empty bind array.
        let md = "```db-postgres alias=q\nSELECT 1\n```\n";
        let doc = make_doc(md);
        let blocks = block_indices(&doc);
        let (sql, binds) = resolve_block_refs(
            doc.segments(),
            blocks[0],
            "SELECT 1 FROM users LIMIT 10",
            &empty_env(),
        )
        .expect("resolves");
        assert_eq!(sql, "SELECT 1 FROM users LIMIT 10");
        assert!(binds.is_empty());
    }

    // ───────────── DB response shim (multi-statement) ─────────────
    //
    // Once a block is a `db-*` block and its cached_result has the
    // `{results: [...]}` shape, `{{alias.response.…}}` enters the
    // shim path that mirrors the desktop's `makeDbResponseView`:
    //   - response.results / response.messages / response.stats: passthrough
    //   - response.<N>: numeric shortcut → results[N]
    //   - response.<col>: legacy → results[0].rows[0].<col>

    #[test]
    fn db_shim_legacy_response_col_resolves_first_row_first_result() {
        // `{{q.response.id}}` ≡ `results[0].rows[0].id` — the
        // pre-redesign shape. Notes that pre-date multi-result must
        // keep working, so this is a parity guarantee.
        let md =
            "```db-postgres alias=src\nSELECT 1\n```\n\n```db-postgres alias=q\nSELECT 1\n```\n";
        let mut doc = make_doc(md);
        let blocks = block_indices(&doc);
        set_cache(
            &mut doc,
            blocks[0],
            db_response(serde_json::json!([select_result(
                serde_json::json!([{ "id": 7, "name": "alice" }])
            ),])),
        );
        let (sql, binds) = resolve_block_refs(
            doc.segments(),
            blocks[1],
            "SELECT * FROM users WHERE id = {{src.response.id}}",
            &empty_env(),
        )
        .expect("resolves");
        assert_eq!(sql, "SELECT * FROM users WHERE id = ?");
        assert_eq!(binds, vec![serde_json::json!(7)]);
    }

    #[test]
    fn db_shim_explicit_path_walks_results_array() {
        // `{{q.response.0.rows.0.id}}` is the shape `{{` autocomplete
        // will guide users toward — passes through `results[]` cleanly.
        let md =
            "```db-postgres alias=src\nSELECT 1\n```\n\n```db-postgres alias=q\nSELECT 1\n```\n";
        let mut doc = make_doc(md);
        let blocks = block_indices(&doc);
        set_cache(
            &mut doc,
            blocks[0],
            db_response(serde_json::json!([select_result(
                serde_json::json!([{ "id": 7 }, { "id": 8 }])
            ),])),
        );
        let (_, binds) = resolve_block_refs(
            doc.segments(),
            blocks[1],
            "SELECT {{src.response.0.rows.1.id}}",
            &empty_env(),
        )
        .expect("resolves");
        assert_eq!(binds, vec![serde_json::json!(8)]);
    }

    #[test]
    fn db_shim_numeric_shortcut_targets_second_result_set() {
        // `BEGIN; SELECT a; SELECT b; ROLLBACK;` → 4 results. The
        // numeric shortcut `response.2` lets a downstream block grab
        // the *second* SELECT without spelling out `results.2`.
        let md =
            "```db-postgres alias=src\nSELECT 1\n```\n\n```db-postgres alias=q\nSELECT 1\n```\n";
        let mut doc = make_doc(md);
        let blocks = block_indices(&doc);
        set_cache(
            &mut doc,
            blocks[0],
            db_response(serde_json::json!([
                serde_json::json!({ "kind": "mutation", "rows_affected": 0 }),
                select_result(serde_json::json!([{ "x": 1 }])),
                select_result(serde_json::json!([{ "y": 99 }])),
                serde_json::json!({ "kind": "mutation", "rows_affected": 0 }),
            ])),
        );
        let (_, binds) = resolve_block_refs(
            doc.segments(),
            blocks[1],
            "SELECT {{src.response.2.rows.0.y}}",
            &empty_env(),
        )
        .expect("resolves");
        assert_eq!(binds, vec![serde_json::json!(99)]);
    }

    #[test]
    fn db_shim_passthrough_stats_returns_elapsed_ms() {
        // `response.stats.elapsed_ms` walks the raw `DbResponse`
        // shape — useful for "did the upstream block take too long?"
        // gating, and proves the passthrough branch is wired.
        let md =
            "```db-postgres alias=src\nSELECT 1\n```\n\n```db-postgres alias=q\nSELECT 1\n```\n";
        let mut doc = make_doc(md);
        let blocks = block_indices(&doc);
        set_cache(
            &mut doc,
            blocks[0],
            db_response(serde_json::json!([select_result(
                serde_json::json!([{ "id": 1 }])
            ),])),
        );
        let (_, binds) = resolve_block_refs(
            doc.segments(),
            blocks[1],
            "SELECT {{src.response.stats.elapsed_ms}}",
            &empty_env(),
        )
        .expect("resolves");
        assert_eq!(binds, vec![serde_json::json!(12)]);
    }

    #[test]
    fn db_shim_mutation_rows_affected_via_explicit_path() {
        // For mutations there's no `rows[]`, so the legacy column
        // shim doesn't apply. The explicit `response.0.rows_affected`
        // path goes through the numeric-shortcut branch and reads it
        // off the result-set object.
        let md = "```db-postgres alias=src\nUPDATE foo SET x=1\n```\n\n```db-postgres alias=q\nSELECT 1\n```\n";
        let mut doc = make_doc(md);
        let blocks = block_indices(&doc);
        set_cache(
            &mut doc,
            blocks[0],
            db_response(serde_json::json!([
                serde_json::json!({ "kind": "mutation", "rows_affected": 7 }),
            ])),
        );
        let (_, binds) = resolve_block_refs(
            doc.segments(),
            blocks[1],
            "SELECT {{src.response.0.rows_affected}}",
            &empty_env(),
        )
        .expect("resolves");
        assert_eq!(binds, vec![serde_json::json!(7)]);
    }

    #[test]
    fn db_shim_legacy_against_mutation_errors_clearly() {
        // `response.<col>` falls through the legacy branch which
        // expects rows[0]. A mutation has no rows, so the user sees a
        // clear error instead of a confusing "column not found".
        let md = "```db-postgres alias=src\nUPDATE foo SET x=1\n```\n\n```db-postgres alias=q\nSELECT 1\n```\n";
        let mut doc = make_doc(md);
        let blocks = block_indices(&doc);
        set_cache(
            &mut doc,
            blocks[0],
            db_response(serde_json::json!([
                serde_json::json!({ "kind": "mutation", "rows_affected": 1 }),
            ])),
        );
        let err = resolve_block_refs(
            doc.segments(),
            blocks[1],
            "SELECT {{src.response.id}}",
            &empty_env(),
        )
        .expect_err("mutation has no rows");
        assert!(
            err.contains("rows") || err.contains("mutation"),
            "got: {err}"
        );
    }

    #[test]
    fn db_shim_out_of_bounds_result_index_errors() {
        // `response.5` against a single-result response surfaces a
        // bounds error with the actual length so users can fix the
        // path.
        let md =
            "```db-postgres alias=src\nSELECT 1\n```\n\n```db-postgres alias=q\nSELECT 1\n```\n";
        let mut doc = make_doc(md);
        let blocks = block_indices(&doc);
        set_cache(
            &mut doc,
            blocks[0],
            db_response(serde_json::json!([select_result(
                serde_json::json!([{ "id": 1 }])
            ),])),
        );
        let err = resolve_block_refs(
            doc.segments(),
            blocks[1],
            "SELECT {{src.response.5.rows.0.id}}",
            &empty_env(),
        )
        .expect_err("only 1 result, idx 5 out of bounds");
        assert!(err.contains("out of bounds"), "got: {err}");
    }

    #[test]
    fn db_shim_skipped_when_cached_lacks_results_array() {
        // Pre-redesign caches don't have `{results: [...]}` — the
        // shim must not engage so older notes still resolve via plain
        // dot-navigation. Here the cached blob is a flat object.
        let md =
            "```db-postgres alias=src\nSELECT 1\n```\n\n```db-postgres alias=q\nSELECT 1\n```\n";
        let mut doc = make_doc(md);
        let blocks = block_indices(&doc);
        set_cache(&mut doc, blocks[0], serde_json::json!({ "id": 42 }));
        let (_, binds) = resolve_block_refs(
            doc.segments(),
            blocks[1],
            "SELECT {{src.response.id}}",
            &empty_env(),
        )
        .expect("resolves via legacy dot-nav");
        assert_eq!(binds, vec![serde_json::json!(42)]);
    }

    // ───────────── Executor params builder (timeout) ────────────────────────

    #[test]
    fn executor_params_includes_timeout_when_set() {
        // `timeout=NNNN` token in the fence flows here as
        // `Some(NNNN)`. Executor's `DbParams.timeout_ms` reads it
        // verbatim and wraps the run in `tokio::time::timeout`.
        let params = build_db_executor_params("conn-1", "SELECT 1", &[], 0, 100, Some(500));
        assert_eq!(params["timeout_ms"], 500);
    }

    #[test]
    fn executor_params_emits_null_timeout_when_absent() {
        // No fence token → field serializes as `null`. Executor
        // falls back to the connection's default timeout (and then
        // to 30s if there's no override either).
        let params = build_db_executor_params("conn-1", "SELECT 1", &[], 0, 100, None);
        assert!(params["timeout_ms"].is_null());
    }

    #[test]
    fn executor_params_passes_bind_values_through() {
        // Bind values land as a JSON array in the same position
        // the executor reads them. Sanity check on the wire shape.
        let binds = vec![serde_json::json!(7), serde_json::json!("alice")];
        let params = build_db_executor_params("conn-1", "SELECT ?, ?", &binds, 0, 50, None);
        assert_eq!(params["bind_values"][0], 7);
        assert_eq!(params["bind_values"][1], "alice");
        assert_eq!(params["fetch_size"], 50);
    }

    // ───────────── Alias edit ──────────────────────────────────

    #[test]
    fn alias_unique_passes_when_no_collision() {
        // Two-block doc where only the first has an alias — picking
        // a fresh name for the second block should succeed.
        let md = "```http alias=existing\nGET /\n```\n\n```db-postgres\nSELECT 1\n```\n";
        let doc = make_doc(md);
        let blocks = block_indices(&doc);
        assert!(validate_alias_unique(&doc, blocks[1], "fresh_name").is_ok());
    }

    #[test]
    fn alias_unique_blocks_collision_with_other_block() {
        // Picking an alias already used by ANOTHER block fails loudly
        // — silent shadowing would hide downstream `{{alias.path}}`
        // resolution from the second block.
        let md = "```http alias=existing\nGET /\n```\n\n```db-postgres\nSELECT 1\n```\n";
        let doc = make_doc(md);
        let blocks = block_indices(&doc);
        let err =
            validate_alias_unique(&doc, blocks[1], "existing").expect_err("collision must error");
        assert!(err.contains("existing"), "got: {err}");
    }

    #[test]
    fn alias_unique_allows_same_block_keeping_its_own_alias() {
        // Editing block #0's alias to its current value (or any value
        // that only matches itself) is fine — we skip the
        // self-comparison so users can edit-with-no-changes without
        // hitting a fake collision.
        let md = "```http alias=existing\nGET /\n```\n";
        let doc = make_doc(md);
        let blocks = block_indices(&doc);
        assert!(validate_alias_unique(&doc, blocks[0], "existing").is_ok());
    }

    // ───── Settings modal validation ─────

    #[test]
    fn parse_optional_u64_empty_returns_none() {
        // Empty input means "clear the field" — confirm path
        // removes the JSON key when this returns Ok(None).
        assert_eq!(parse_optional_u64(""), Ok(None));
    }

    #[test]
    fn parse_optional_u64_accepts_zero_and_large() {
        assert_eq!(parse_optional_u64("0"), Ok(Some(0)));
        assert_eq!(parse_optional_u64("500"), Ok(Some(500)));
        assert_eq!(parse_optional_u64("4294967296"), Ok(Some(4_294_967_296)));
    }

    #[test]
    fn parse_optional_u64_rejects_non_numeric() {
        assert!(parse_optional_u64("abc").is_err());
        assert!(parse_optional_u64("12.5").is_err());
        assert!(parse_optional_u64("-1").is_err());
        assert!(parse_optional_u64("3 4").is_err());
    }

    #[test]
    fn db_settings_focus_cycle_db() {
        use crate::app::{DbSettingsState, SettingsField};
        use crate::vim::lineedit::LineEdit;
        // DB modal has both limit + timeout; Tab cycles between them.
        let mut s = DbSettingsState {
            segment_idx: 0,
            fields: vec![
                SettingsField {
                    label: "Limit",
                    key: "limit",
                    input: LineEdit::new(),
                },
                SettingsField {
                    label: "Timeout",
                    key: "timeout_ms",
                    input: LineEdit::new(),
                },
            ],
            focus: 0,
        };
        s.focus_next();
        assert_eq!(s.focus, 1);
        s.focus_next();
        assert_eq!(s.focus, 0); // wraps
        s.focus_prev();
        assert_eq!(s.focus, 1); // wraps backwards
    }

    #[test]
    fn preview_sql_collapses_whitespace_and_truncates() {
        // Multi-line SQL with tabs/CRs gets folded to single-space
        // for the history modal's one-line cell. No trailing space.
        let sql = "SELECT *\n  FROM users\nWHERE id = 1";
        assert_eq!(preview_sql(sql), "SELECT * FROM users WHERE id = 1");
    }

    #[test]
    fn preview_sql_truncates_with_ellipsis() {
        // Strings longer than the 200-char cap get truncated and
        // suffixed with `…` so the user knows there's more.
        let long_sql = "SELECT ".to_string() + &"col_name, ".repeat(40) + "FROM huge_table";
        let preview = preview_sql(&long_sql);
        assert!(
            preview.chars().count() <= 201,
            "got len {}",
            preview.chars().count()
        );
        assert!(preview.ends_with('…'));
    }

    #[test]
    fn preview_sql_short_unchanged() {
        // Short SQL passes through verbatim (modulo whitespace
        // collapse) so the common case isn't decorated.
        let sql = "SELECT 1";
        let preview = preview_sql(sql);
        assert_eq!(preview, "SELECT 1");
        assert!(!preview.ends_with('…'));
    }

    #[test]
    fn db_settings_focus_cycle_http_is_noop() {
        use crate::app::{DbSettingsState, SettingsField};
        use crate::vim::lineedit::LineEdit;
        // HTTP modal has only timeout — Tab is a no-op (focus
        // stays at index 0 regardless of direction).
        let mut s = DbSettingsState {
            segment_idx: 0,
            fields: vec![SettingsField {
                label: "Timeout",
                key: "timeout_ms",
                input: LineEdit::new(),
            }],
            focus: 0,
        };
        s.focus_next();
        assert_eq!(s.focus, 0);
        s.focus_prev();
        assert_eq!(s.focus, 0);
    }
}
