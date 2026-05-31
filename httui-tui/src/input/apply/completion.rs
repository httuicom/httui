// coverage:exclude file — legacy vim engine relocated by tui-V1/Fase1
// (behavior-identical, suite-proven); coverage tracked in
// docs-llm/tui-v2/vim-coverage-debt.md (2026-05-19), paid by dedicated épico.
//! SQL/ref completion-popup + DB-confirm appliers. Mechanically moved
//! out of `vim/dispatch.rs` (tui-v2 vertical 1, fase 1 p5c) with no
//! logic change.

use crate::app::{App, StatusKind};
use crate::buffer::{Cursor, Segment};
use crate::commands::db::{load_active_env_vars, resolve_connection_id_sync};
use crate::input::action::Action;

/// Map a key event to a `CompletionPopup*` action, or `None` if the
/// key isn't one the popup wants to claim. Caller (the dispatcher)
/// only calls this when a popup is open; an unrecognized key falls
/// through to mode parsing and the post-action trigger reopens the
/// popup with the new prefix.
pub(crate) fn parse_completion_popup_key(key: crossterm::event::KeyEvent) -> Option<Action> {
    use crossterm::event::{KeyCode, KeyModifiers};
    let plain = key.modifiers == KeyModifiers::NONE;
    let ctrl = key.modifiers == KeyModifiers::CONTROL;
    match key.code {
        KeyCode::Esc => Some(Action::CompletionDismiss),
        KeyCode::Char('c') if ctrl => Some(Action::CompletionDismiss),
        KeyCode::Tab if plain => Some(Action::CompletionAccept),
        KeyCode::Enter if plain => Some(Action::CompletionAccept),
        KeyCode::Down if plain => Some(Action::CompletionNext),
        KeyCode::Up if plain => Some(Action::CompletionPrev),
        KeyCode::Char('n') if ctrl => Some(Action::CompletionNext),
        KeyCode::Char('p') if ctrl => Some(Action::CompletionPrev),
        _ => None,
    }
}

/// Recompute the completion popup against the cursor's current
/// position. Called after every keystroke that may have shifted the
/// prefix word (insert, backspace). Closes the popup when:
/// - cursor isn't in a DB block body, or
/// - prefix is empty, or
/// - no candidates match (avoids painting an empty popup).
pub(crate) fn refresh_completion_popup(app: &mut App) {
    rebuild_completion_popup(app, /* allow_empty_prefix = */ false);
}

/// `Ctrl+Space` — manual trigger. Opens the popup even when the
/// cursor sits right after a non-word char (where there's no prefix
/// yet) so the user can browse the full dialect listing without
/// having to type the first letter. Inside a partial word it
/// behaves like the auto-trigger and shows the filtered list.
pub(crate) fn force_open_completion_popup(app: &mut App) {
    rebuild_completion_popup(app, /* allow_empty_prefix = */ true);
}

/// Shared body for both the auto trigger and the manual one. The
/// only knob is whether an empty prefix is acceptable — auto closes
/// the popup, manual opens it with the full dialect listing.
pub(crate) fn rebuild_completion_popup(app: &mut App, allow_empty_prefix: bool) {
    // BLOCKS view EDIT: the doc redirect points at the sub-doc
    // (`pane.block_edit.doc`), so the cursor is `InProse` not
    // `InBlock`. Detect that here and route to the helper that
    // knows how to extract body + line/col from the sub-doc plus
    // pull the block node from the pane.
    if blocks_edit_completion_active(app) {
        rebuild_completion_popup_blocks_edit(app, allow_empty_prefix);
        return;
    }
    let Some(doc) = app.document() else {
        if matches!(app.modal, Some(crate::modal::Modal::CompletionPopup(_))) {
            app.modal = None;
        }
        return;
    };
    let Cursor::InBlock {
        segment_idx,
        offset: raw_offset,
    } = doc.cursor()
    else {
        if matches!(app.modal, Some(crate::modal::Modal::CompletionPopup(_))) {
            app.modal = None;
        }
        return;
    };
    let Some(seg) = doc.segments().get(segment_idx) else {
        if matches!(app.modal, Some(crate::modal::Modal::CompletionPopup(_))) {
            app.modal = None;
        }
        return;
    };
    let Segment::Block(block) = seg else {
        if matches!(app.modal, Some(crate::modal::Modal::CompletionPopup(_))) {
            app.modal = None;
        }
        return;
    };
    // Ref completion (`{{...}}`) works in HTTP and DB; SQL completion
    // only in DB. Other block kinds get no popup.
    let is_db = block.is_db();
    let is_http = block.is_http();
    if !is_db && !is_http {
        if matches!(app.modal, Some(crate::modal::Modal::CompletionPopup(_))) {
            app.modal = None;
        }
        return;
    }
    // The completion engine speaks body `(line, col)` — convert the
    // cursor's raw-rope offset to body coords and bail out on
    // header / closer rows (no completion in fence text).
    let (line, offset) = match crate::buffer::block::raw_section_at(&block.raw, raw_offset) {
        crate::buffer::block::RawSection::Body { line, col } => (line, col),
        _ => {
            if matches!(app.modal, Some(crate::modal::Modal::CompletionPopup(_))) {
                app.modal = None;
            }
            return;
        }
    };
    // DB blocks expose a parsed `query`; HTTP keeps the request body
    // distributed across params, so we fall back to the raw body text
    // (everything between header and closer). Both produce the same
    // (line, col)-addressable string the completion engine expects.
    let body = if is_db {
        match block.params.get("query").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => {
                if matches!(app.modal, Some(crate::modal::Modal::CompletionPopup(_))) {
                    app.modal = None;
                }
                return;
            }
        }
    } else {
        crate::buffer::block::body_text(&block.raw)
    };
    // Refs win over SQL completion: when the cursor sits inside an
    // open `{{...}}` ref, the popup switches to alias / env-var /
    // ref-path mode entirely. The prefix the popup tracks is what's
    // typed since the last `{{` or `.` — same accept semantics as
    // the SQL path (backspace prefix, splice label).
    if let Some(ref_detect) = crate::sql_completion::detect_ref_context(&body, line, offset) {
        // Need env vars to populate top-level ref candidates.
        let env_vars: std::collections::HashMap<String, String> =
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current()
                    .block_on(load_active_env_vars(&app.environments_store))
            })
            .unwrap_or_default();
        let segments_snapshot: Vec<crate::buffer::Segment> = app
            .document()
            .map(|d| d.segments().to_vec())
            .unwrap_or_default();
        let items = crate::sql_completion::complete_refs(
            &ref_detect,
            &segments_snapshot,
            segment_idx,
            &env_vars,
        );
        if items.is_empty() {
            if matches!(app.modal, Some(crate::modal::Modal::CompletionPopup(_))) {
                app.modal = None;
            }
            return;
        }
        let prior_label = app
            .completion_popup()
            .and_then(|p| p.items.get(p.selected))
            .map(|i| i.label.clone());
        let selected = prior_label
            .and_then(|lbl| items.iter().position(|i| i.label == lbl))
            .unwrap_or(0);
        app.modal = Some(crate::modal::Modal::CompletionPopup(
            crate::app::CompletionPopupState {
                segment_idx,
                items,
                selected,
                anchor_line: line,
                anchor_offset: ref_detect.anchor_offset,
                prefix: ref_detect.prefix,
            },
        ));
        return;
    }

    // Outside `{{...}}`, only DB blocks have an SQL completion path —
    // HTTP / other kinds close the popup.
    if !is_db {
        if matches!(app.modal, Some(crate::modal::Modal::CompletionPopup(_))) {
            app.modal = None;
        }
        return;
    }

    // Detect the prefix word at the cursor. When we got there via a
    // manual trigger and the cursor isn't on a word char, fall back
    // to "no prefix, anchor at cursor" so the popup still opens.
    let (anchor_offset, prefix) = match crate::sql_completion::prefix_at_cursor(&body, line, offset)
    {
        Some(p) => p,
        None if allow_empty_prefix => (offset, String::new()),
        None => {
            if matches!(app.modal, Some(crate::modal::Modal::CompletionPopup(_))) {
                app.modal = None;
            }
            return;
        }
    };
    let dialect = crate::sql_completion::Dialect::from_block(block);
    let context = crate::sql_completion::detect_context(&body, line, anchor_offset);
    // The fence may carry either a UUID (canonical, written by the
    // picker) or a human slug (legacy / hand-typed). The schema
    // cache is always keyed by UUID, so resolve here via the
    // `connection_names` map (id → name) — a reverse scan finds the
    // id when the fence has a slug.
    let conn_raw = block
        .params
        .get("connection")
        .or_else(|| block.params.get("connection_id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let conn_id = conn_raw
        .as_deref()
        .map(|raw| resolve_connection_id_sync(raw, &app.connection_names));
    // Borrow of `block` ends here — the snapshot below clones the
    // schema tables so we can mutate `app` after.
    let schema_tables: Option<Vec<crate::schema::SchemaTable>> = conn_id
        .as_deref()
        .and_then(|id| app.schema_cache.get(id))
        .map(|e| e.tables.clone());
    // First-time popup on this connection? Kick off the background
    // introspection now. The fetch is idempotent + dedup'd, so
    // calling it again on every keystroke is free; the result lands
    // via `AppEvent::SchemaLoaded` and the next popup refresh sees
    // it cached.
    if let Some(id) = conn_id.as_deref() {
        if schema_tables.is_none() {
            app.ensure_schema_loaded(id);
        }
    }
    let items =
        crate::sql_completion::complete(dialect, &prefix, context, schema_tables.as_deref());
    if items.is_empty() {
        if matches!(app.modal, Some(crate::modal::Modal::CompletionPopup(_))) {
            app.modal = None;
        }
        return;
    }
    // Preserve the previous selection's label so re-filtering on a
    // longer prefix doesn't reset the highlight to the top — useful
    // when the user is typing toward a known target.
    let prior_label = app
        .completion_popup()
        .and_then(|p| p.items.get(p.selected))
        .map(|i| i.label.clone());
    let selected = prior_label
        .and_then(|lbl| items.iter().position(|i| i.label == lbl))
        .unwrap_or(0);
    app.modal = Some(crate::modal::Modal::CompletionPopup(
        crate::app::CompletionPopupState {
            segment_idx,
            items,
            selected,
            anchor_line: line,
            anchor_offset,
            prefix,
        },
    ));
}

// All DB-domain helpers (`build_db_executor_params`,
// `compute_db_cache_hash`, the SQL classifiers, ref/bind resolvers,
// `summarize_db_response`, `load_active_env_vars`, `resolve_connection_id`,
// `resolve_connection_id_sync`, `save_db_cache_async`,
// `db_summary_from_value`) moved to `crate::commands::db`. The imports
// at the top bring them into local scope so the existing
// `apply_run_block` / `handle_db_block_result` / `rebuild_completion_popup`
// call sites keep working until those flows migrate too (next session).

/// `y` (or `Enter`) inside the run-confirm modal — close the
/// modal, drop back to normal mode, then re-run the original
/// segment with the unscoped-destructive gate bypassed. The
/// segment_idx comes from the modal state because the cursor may
/// have moved while the modal was up.
pub(crate) fn apply_confirm_db_run(app: &mut App) {
    let segment_idx = match app.modal.take() {
        Some(crate::modal::Modal::ConfirmPrompt(state)) => match state.payload {
            crate::app::ConfirmPayload::DbSegment(idx) => Some(idx),
            other => {
                // Wrong-payload prompt was open — re-store and bail.
                app.modal = Some(crate::modal::Modal::ConfirmPrompt(
                    crate::app::ConfirmPromptState {
                        payload: other,
                        ..state
                    },
                ));
                None
            }
        },
        other => {
            app.modal = other;
            None
        }
    };
    let Some(segment_idx) = segment_idx else {
        app.vim.enter_normal();
        return;
    };
    app.vim.enter_normal();
    crate::commands::db::run_db_block_inner(app, segment_idx, true, None, false);
}

pub(crate) fn apply_cancel_db_run(app: &mut App) {
    if matches!(app.modal, Some(crate::modal::Modal::ConfirmPrompt(_))) {
        app.modal = None;
        app.set_status(StatusKind::Info, "run cancelled");
    }
    app.vim.enter_normal();
}

pub(crate) fn apply_completion_next(app: &mut App) {
    let Some(state) = app.completion_popup_mut() else {
        return;
    };
    if state.items.is_empty() {
        return;
    }
    state.selected = (state.selected + 1) % state.items.len();
}

pub(crate) fn apply_completion_prev(app: &mut App) {
    let Some(state) = app.completion_popup_mut() else {
        return;
    };
    if state.items.is_empty() {
        return;
    }
    state.selected = if state.selected == 0 {
        state.items.len() - 1
    } else {
        state.selected - 1
    };
}

/// True when any BLOCKS-view EDIT buffer is active. SQL completion only
/// fires on DB query (gated downstream by `block_type.starts_with("db")`);
/// `{{ref}}` completion works in every field — URL, header value, HTTP
/// body, DB query — since refs are uniform across block kinds.
fn blocks_edit_completion_active(app: &App) -> bool {
    if !matches!(app.view, crate::app::AppView::Blocks) {
        return false;
    }
    let Some(pane) = app.active_pane() else {
        return false;
    };
    pane.block_edit.is_some()
}

/// `rebuild_completion_popup` for BLOCKS view EDIT on a DB Query.
/// Mirrors the DOC view path but reads body + cursor from the
/// sub-doc rather than `Cursor::InBlock` on the pane document.
fn rebuild_completion_popup_blocks_edit(app: &mut App, allow_empty_prefix: bool) {
    // Pull the sub-doc text + cursor (line, col) from the edit
    // buffer the user is typing into. Also snapshot the file-wide
    // segments + the focused block's index so ref completion sees
    // every block above this one (not just the field sub-doc, which
    // has zero `Segment::Block` entries).
    let (body, line, offset, block_type, conn_raw, mut file_segments, current_segment, abs_path) = {
        let Some(pane) = app.active_pane() else {
            close_completion_popup_if_open(app);
            return;
        };
        let Some(edit) = pane.block_edit.as_ref() else {
            close_completion_popup_if_open(app);
            return;
        };
        let body = edit.current_text();
        let cursor_offset = match edit.doc.cursor() {
            crate::buffer::Cursor::InProse { offset, .. } => offset,
            crate::buffer::Cursor::InBlock { offset, .. } => offset,
            _ => 0,
        };
        let mut line = 0usize;
        let mut col = 0usize;
        for ch in body.chars().take(cursor_offset) {
            if ch == '\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        // Resolve the block_type + connection raw from the workspace
        // index — we need both to pick the dialect and the schema.
        let Some(ws) = app.blocks_workspace.as_ref() else {
            close_completion_popup_if_open(app);
            return;
        };
        let Some(sel) = pane.block_selected else {
            close_completion_popup_if_open(app);
            return;
        };
        let Some(file) = ws.index.files.get(sel.file_idx) else {
            close_completion_popup_if_open(app);
            return;
        };
        let Some(meta) = file.blocks.get(sel.block_idx) else {
            close_completion_popup_if_open(app);
            return;
        };
        let block_type = meta.block_type.clone();
        let alias = meta.alias.clone();
        let conn_raw = read_block_connection(&app.vault_path, &file.path, meta.line_start);
        let (file_segments, current_segment) = pane
            .document
            .as_ref()
            .map(|doc| {
                let segs: Vec<crate::buffer::Segment> = doc.segments().to_vec();
                let idx = segs
                    .iter()
                    .position(|s| matches!(
                        s,
                        crate::buffer::Segment::Block(b)
                            if b.block_type == block_type && b.alias == alias
                    ))
                    .unwrap_or(segs.len());
                (segs, idx)
            })
            .unwrap_or_else(|| (Vec::new(), 0));
        let abs_path = app.vault_path.join(&file.path);
        (
            body,
            line,
            col,
            block_type,
            conn_raw,
            file_segments,
            current_segment,
            abs_path,
        )
    };

    // Re-attach `cached_result` from SQLite onto the segments snapshot
    // so the popup sees the latest captured response — another pane (or
    // a previous session) may have refreshed it after this pane's
    // `pane.document` was last loaded. Without this the popup paths
    // would drift after every cross-pane rerun.
    let env_vars_for_hydrate: std::collections::HashMap<String, String> =
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(load_active_env_vars(&app.environments_store))
        })
        .unwrap_or_default();
    crate::block_hydrate::hydrate_segments_blocking(
        app.pool_manager.app_pool(),
        &mut file_segments,
        &env_vars_for_hydrate,
        &abs_path,
    );

    // Stable anchor key for the popup state — the renderer overrides
    // position via `popup_cursor_cell` published from the BLOCKS pane,
    // so this index just keys the modal.
    let segment_idx = 0usize;

    // Ref completion (`{{...}}`) — works in any block kind. Same
    // engine the DOC view uses.
    if let Some(ref_detect) = crate::sql_completion::detect_ref_context(&body, line, offset) {
        let items = crate::sql_completion::complete_refs(
            &ref_detect,
            &file_segments,
            current_segment,
            &env_vars_for_hydrate,
        );
        if items.is_empty() {
            close_completion_popup_if_open(app);
            return;
        }
        let prior_label = app
            .completion_popup()
            .and_then(|p| p.items.get(p.selected))
            .map(|i| i.label.clone());
        let selected = prior_label
            .and_then(|lbl| items.iter().position(|i| i.label == lbl))
            .unwrap_or(0);
        app.modal = Some(crate::modal::Modal::CompletionPopup(
            crate::app::CompletionPopupState {
                segment_idx,
                items,
                selected,
                anchor_line: line,
                anchor_offset: ref_detect.anchor_offset,
                prefix: ref_detect.prefix,
            },
        ));
        return;
    }

    // SQL completion — DB blocks only.
    if !block_type.starts_with("db") {
        close_completion_popup_if_open(app);
        return;
    }
    let (anchor_offset, prefix) =
        match crate::sql_completion::prefix_at_cursor(&body, line, offset) {
            Some(p) => p,
            None if allow_empty_prefix => (offset, String::new()),
            None => {
                close_completion_popup_if_open(app);
                return;
            }
        };
    let dialect = crate::sql_completion::Dialect::from_block_type(&block_type);
    let context = crate::sql_completion::detect_context(&body, line, anchor_offset);
    let conn_id = conn_raw
        .as_deref()
        .map(|raw| resolve_connection_id_sync(raw, &app.connection_names));
    let schema_tables: Option<Vec<crate::schema::SchemaTable>> = conn_id
        .as_deref()
        .and_then(|id| app.schema_cache.get(id))
        .map(|e| e.tables.clone());
    if let Some(id) = conn_id.as_deref() {
        if schema_tables.is_none() {
            app.ensure_schema_loaded(id);
        }
    }
    let items =
        crate::sql_completion::complete(dialect, &prefix, context, schema_tables.as_deref());
    if items.is_empty() {
        close_completion_popup_if_open(app);
        return;
    }
    let prior_label = app
        .completion_popup()
        .and_then(|p| p.items.get(p.selected))
        .map(|i| i.label.clone());
    let selected = prior_label
        .and_then(|lbl| items.iter().position(|i| i.label == lbl))
        .unwrap_or(0);
    app.modal = Some(crate::modal::Modal::CompletionPopup(
        crate::app::CompletionPopupState {
            segment_idx,
            items,
            selected,
            anchor_line: line,
            anchor_offset,
            prefix,
        },
    ));
}

fn close_completion_popup_if_open(app: &mut App) {
    if matches!(app.modal, Some(crate::modal::Modal::CompletionPopup(_))) {
        app.modal = None;
    }
}

fn read_block_connection(
    vault: &std::path::Path,
    file: &std::path::Path,
    line_start: usize,
) -> Option<String> {
    let text =
        httui_core::fs::read_note(&vault.to_string_lossy(), &file.to_string_lossy()).ok()?;
    let parsed = httui_core::blocks::parse_blocks(&text);
    let p = parsed.iter().find(|p| p.line_start == line_start)?;
    p.params
        .get("connection")
        .or_else(|| p.params.get("connection_id"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

pub(crate) fn apply_completion_dismiss(app: &mut App) {
    if matches!(app.modal, Some(crate::modal::Modal::CompletionPopup(_))) {
        app.modal = None;
    }
    // Mirror nvim: Esc on an open completion popup both dismisses
    // the popup AND drops Insert → Normal. The user typed `<Esc>`
    // intending "exit"; doing only half is jarring.
    if app.config.editor.mode == crate::config::EditorMode::Vim
        && matches!(app.vim.mode, crate::vim::mode::Mode::Insert)
    {
        app.vim.enter_normal();
    }
}

/// Splice the selected item's label in place of the prefix word at
/// the cursor. Implementation: backspace `prefix.len()` characters
/// (which clears the partial word in the body), then insert each
/// char of the label. Cursor lands at the end of the inserted text.
pub(crate) fn apply_completion_accept(app: &mut App) {
    let state = match app.modal.take() {
        Some(crate::modal::Modal::CompletionPopup(s)) => s,
        other => {
            app.modal = other;
            return;
        }
    };
    let Some(item) = state.items.get(state.selected).cloned() else {
        return;
    };
    let prefix_chars = state.prefix.chars().count();
    let Some(doc) = app.document_mut() else {
        return;
    };
    doc.snapshot();
    for _ in 0..prefix_chars {
        doc.delete_char_before_cursor();
    }
    for ch in item.label.chars() {
        doc.insert_char_at_cursor(ch);
    }
}

/// `apply_action` sub-match for the completion-popup + DB-confirm
/// domain. Mechanically split out of the `apply_action` router in
/// `vim/dispatch.rs` (tui-v2 vertical 1, fase 1 p6b) — arm bodies are
/// copied verbatim. The outer router routes only this group's variants
/// here, so the `unreachable!` is a compile-time-backed invariant.
pub(crate) fn apply_completion(app: &mut App, action: Action, _recording: bool) {
    match action {
        Action::CompletionNext => apply_completion_next(app),
        Action::CompletionPrev => apply_completion_prev(app),
        Action::CompletionAccept => apply_completion_accept(app),
        Action::CompletionDismiss => apply_completion_dismiss(app),
        Action::ConfirmDbRun => apply_confirm_db_run(app),
        Action::CancelDbRun => apply_cancel_db_run(app),
        _ => unreachable!("apply_completion: variante fora do grupo"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Segment;
    use crate::config::{Config, EditorMode};
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use tempfile::TempDir;

    async fn app_with(md: &str) -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        std::fs::write(vault.path().join("note.md"), md).unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let mut cfg = Config::default();
        cfg.editor.mode = EditorMode::Standard;
        let app = App::new(cfg, resolved, pool);
        (app, data, vault)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn http_block_opens_ref_popup_inside_double_brace() {
        // Regression: ref autocomplete (`{{`) used to be DB-only — the
        // rebuild_completion_popup short-circuited on !is_db. HTTP now
        // shares the ref completion path so `{{` inside a header value
        // or URL surfaces aliases (and env keys) just like in DB.
        // The fixture includes an earlier block with `alias=ping` so
        // complete_refs has at least one candidate to surface.
        let md = "```http alias=ping\nGET https://x\n```\n\n```http alias=req\nGET https://api.example.com\nAuthorization: Bearer {{\n```\n";
        let (mut app, _d, _v) = app_with(md).await;
        // Pick the SECOND block (alias=req) — that's where we'll type.
        let block_idx = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .enumerate()
            .filter(|(_, s)| matches!(s, Segment::Block(_)))
            .nth(1)
            .map(|(i, _)| i)
            .expect("fixture has two http blocks");

        let raw = match app.document().unwrap().segments().get(block_idx) {
            Some(Segment::Block(b)) => b.raw.to_string(),
            _ => unreachable!(),
        };
        let off = raw.find("{{").map(|i| i + 2).unwrap();
        app.document_mut().unwrap().set_cursor(Cursor::InBlock {
            segment_idx: block_idx,
            offset: off,
        });

        rebuild_completion_popup(&mut app, /* allow_empty_prefix = */ true);

        let popup = app
            .completion_popup()
            .expect("popup should open for {{ context in HTTP block");
        assert_eq!(popup.segment_idx, block_idx);
        assert!(
            popup.prefix.is_empty(),
            "prefix should be empty right after `{{{{`, got {:?}",
            popup.prefix
        );
        // And the earlier alias must show up as a candidate.
        assert!(
            popup.items.iter().any(|i| i.label == "ping"),
            "popup should list the earlier `ping` alias, got {:?}",
            popup.items.iter().map(|i| &i.label).collect::<Vec<_>>()
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn http_block_closes_popup_outside_double_brace() {
        // Outside `{{...}}`, HTTP has no completion path (no SQL
        // dialect). Popup must NOT open.
        let md = "```http alias=req\nGET https://api.example.com\n```\n";
        let (mut app, _d, _v) = app_with(md).await;
        let block_idx = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .expect("fixture has an http block");
        let raw = match app.document().unwrap().segments().get(block_idx) {
            Some(Segment::Block(b)) => b.raw.to_string(),
            _ => unreachable!(),
        };
        let off = raw.find("GET ").map(|i| i + 4).unwrap();
        app.document_mut().unwrap().set_cursor(Cursor::InBlock {
            segment_idx: block_idx,
            offset: off,
        });

        rebuild_completion_popup(&mut app, true);
        assert!(
            app.completion_popup().is_none(),
            "HTTP plain text shouldn't open the SQL popup"
        );
    }
}
