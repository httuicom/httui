//! Detail-modal key handlers (DB row + HTTP response detail).
//!
//! Detail modals are different from every other [`Modal`](super::Modal)
//! variant: their body is a sub-`Document` and the user navigates it
//! with the full vim motion engine (read-only). The handler here owns
//! two responsibilities:
//!
//! 1. **Modal-specific shortcuts** — `Ctrl-c` closes; `Y` (uppercase)
//!    copies the row (DB) or response body (HTTP) to the clipboard.
//!    `Esc`/`q` keep their normal vim semantics (clear pending chord /
//!    start macro recording) so a stray `Esc` during a `yi{` chord
//!    doesn't teleport-close the modal.
//! 2. **Routing decision** — when *not* owning the key, return
//!    [`ModalOutcome::Forward`] so the scope walker delegates to the
//!    editor scope below. `App::document_mut` redirects to the modal's
//!    sub-doc, so the editor's motions land inside the modal:
//!    - Standard editor profile → forward; `route_standard` operates
//!      on the redirected doc.
//!    - Vim profile but `vim.mode` is a transient (Visual, Search,
//!      CmdLine) → forward; the vim dispatcher routes to `parse_visual`
//!      etc. which restores `Mode::DbRowDetail` after the transient
//!      mode exits via `visual_origin_mode`.
//!    - Vim profile sitting in `Mode::DbRowDetail` → modal owns; key
//!      goes through `parse_db_row_detail` (the read-only filter on
//!      `parse_normal`).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::config::EditorMode;
use crate::input::action::Action;
use crate::vim::mode::Mode;

use super::{ModalKeyCtx, ModalOutcome};

/// Standard-mode shortcuts for detail modals: `Ctrl+C` copies (the
/// JSON row / HTTP body) and `Esc` closes. Without intercepting
/// these here, `Ctrl+C` would fall through to `route_standard` and
/// quit the TUI; `Esc` would just clear selection.
fn standard_detail_shortcut(key: KeyEvent, copy: Action, close: Action) -> Option<Action> {
    match (key.modifiers, key.code) {
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => Some(copy),
        (KeyModifiers::NONE, KeyCode::Esc) => Some(close),
        _ => None,
    }
}

/// `Modal::DbRowDetail.handle_key_with_ctx` body. See module docs for
/// the routing contract.
pub(super) fn db_row_handle_key(key: KeyEvent, ctx: &mut ModalKeyCtx<'_>) -> ModalOutcome {
    if matches!(ctx.editor_mode, EditorMode::Standard) {
        if let Some(a) = standard_detail_shortcut(
            key,
            Action::CopyDbRowDetailJson,
            Action::CloseDbRowDetail,
        ) {
            return ModalOutcome::Emit(a);
        }
        return ModalOutcome::Forward;
    }
    if ctx.vim.mode != Mode::DbRowDetail {
        return ModalOutcome::Forward;
    }
    let action = crate::input::parser::modals::parse_db_row_detail(ctx.vim, key);
    ModalOutcome::Emit(action)
}

/// `Modal::HttpResponseDetail.handle_key_with_ctx` body. Mirrors
/// [`db_row_handle_key`].
pub(super) fn http_response_handle_key(key: KeyEvent, ctx: &mut ModalKeyCtx<'_>) -> ModalOutcome {
    if matches!(ctx.editor_mode, EditorMode::Standard) {
        if let Some(a) = standard_detail_shortcut(
            key,
            Action::CopyHttpResponseBody,
            Action::CloseHttpResponseDetail,
        ) {
            return ModalOutcome::Emit(a);
        }
        return ModalOutcome::Forward;
    }
    if ctx.vim.mode != Mode::HttpResponseDetail {
        return ModalOutcome::Forward;
    }
    let action = crate::input::parser::modals::parse_http_response_detail(ctx.vim, key);
    ModalOutcome::Emit(action)
}
