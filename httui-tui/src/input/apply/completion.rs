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
    let Some(doc) = app.document() else {
        app.completion_popup = None;
        return;
    };
    let Cursor::InBlock {
        segment_idx,
        offset: raw_offset,
    } = doc.cursor()
    else {
        app.completion_popup = None;
        return;
    };
    let Some(seg) = doc.segments().get(segment_idx) else {
        app.completion_popup = None;
        return;
    };
    let Segment::Block(block) = seg else {
        app.completion_popup = None;
        return;
    };
    if !block.is_db() {
        app.completion_popup = None;
        return;
    }
    // The completion engine speaks body `(line, col)` — convert the
    // cursor's raw-rope offset to body coords and bail out on
    // header / closer rows (no completion in fence text).
    let (line, offset) = match crate::buffer::block::raw_section_at(&block.raw, raw_offset) {
        crate::buffer::block::RawSection::Body { line, col } => (line, col),
        _ => {
            app.completion_popup = None;
            return;
        }
    };
    let body = match block.params.get("query").and_then(|v| v.as_str()) {
        Some(s) => s.to_string(),
        None => {
            app.completion_popup = None;
            return;
        }
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
                    .block_on(load_active_env_vars(app.pool_manager.app_pool()))
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
            app.completion_popup = None;
            return;
        }
        let prior_label = app
            .completion_popup
            .as_ref()
            .and_then(|p| p.items.get(p.selected))
            .map(|i| i.label.clone());
        let selected = prior_label
            .and_then(|lbl| items.iter().position(|i| i.label == lbl))
            .unwrap_or(0);
        app.completion_popup = Some(crate::app::CompletionPopupState {
            segment_idx,
            items,
            selected,
            anchor_line: line,
            anchor_offset: ref_detect.anchor_offset,
            prefix: ref_detect.prefix,
        });
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
            app.completion_popup = None;
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
        app.completion_popup = None;
        return;
    }
    // Preserve the previous selection's label so re-filtering on a
    // longer prefix doesn't reset the highlight to the top — useful
    // when the user is typing toward a known target.
    let prior_label = app
        .completion_popup
        .as_ref()
        .and_then(|p| p.items.get(p.selected))
        .map(|i| i.label.clone());
    let selected = prior_label
        .and_then(|lbl| items.iter().position(|i| i.label == lbl))
        .unwrap_or(0);
    app.completion_popup = Some(crate::app::CompletionPopupState {
        segment_idx,
        items,
        selected,
        anchor_line: line,
        anchor_offset,
        prefix,
    });
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
    let Some(state) = app.db_confirm_run.take() else {
        app.vim.enter_normal();
        return;
    };
    app.vim.enter_normal();
    crate::commands::db::run_db_block_inner(
        app,
        state.segment_idx,
        /* force_unscoped = */ true,
        None,
        /* as_explain = */ false,
    );
}

/// `n` / `Esc` / `Ctrl-C` — close the modal without running.
pub(crate) fn apply_cancel_db_run(app: &mut App) {
    if app.db_confirm_run.take().is_some() {
        app.set_status(StatusKind::Info, "run cancelled");
    }
    app.vim.enter_normal();
}

pub(crate) fn apply_completion_next(app: &mut App) {
    let Some(state) = app.completion_popup.as_mut() else {
        return;
    };
    if state.items.is_empty() {
        return;
    }
    state.selected = (state.selected + 1) % state.items.len();
}

pub(crate) fn apply_completion_prev(app: &mut App) {
    let Some(state) = app.completion_popup.as_mut() else {
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

pub(crate) fn apply_completion_dismiss(app: &mut App) {
    app.completion_popup = None;
}

/// Splice the selected item's label in place of the prefix word at
/// the cursor. Implementation: backspace `prefix.len()` characters
/// (which clears the partial word in the body), then insert each
/// char of the label. Cursor lands at the end of the inserted text.
pub(crate) fn apply_completion_accept(app: &mut App) {
    let Some(state) = app.completion_popup.take() else {
        return;
    };
    let Some(item) = state.items.get(state.selected).cloned() else {
        return;
    };
    let prefix_chars = state.prefix.chars().count();
    let Some(doc) = app.tabs.active_document_mut() else {
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
