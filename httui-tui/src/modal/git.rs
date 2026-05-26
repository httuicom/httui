//! Per-key handlers for the git-specific modals. Split out of
//! `modal/handlers.rs` to keep that file under the size gate; all
//! three handlers reuse `super::handlers::*` primitives.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::input::action::Action;

use super::handlers::{list_picker_key, ListPickerKey};
use super::ModalOutcome;

/// j/k/arrows navigate commits; Esc closes; PageUp/PageDown scroll
/// the diff pane.
pub(super) fn log_page_handle_key(key: KeyEvent) -> ModalOutcome {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) => ModalOutcome::Emit(Action::CloseGitLogPage),
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => ModalOutcome::Emit(Action::CloseGitLogPage),
        (_, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => {
            ModalOutcome::Emit(Action::MoveGitLogPageCursor(1))
        }
        (_, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
            ModalOutcome::Emit(Action::MoveGitLogPageCursor(-1))
        }
        (_, KeyCode::PageDown) => ModalOutcome::Emit(Action::ScrollGitLogDiff(10)),
        (_, KeyCode::PageUp) => ModalOutcome::Emit(Action::ScrollGitLogDiff(-10)),
        (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
            ModalOutcome::Emit(Action::ScrollGitLogDiff(10))
        }
        (KeyModifiers::CONTROL, KeyCode::Char('u')) => {
            ModalOutcome::Emit(Action::ScrollGitLogDiff(-10))
        }
        _ => ModalOutcome::Continue,
    }
}

/// j/k/arrows navigate; Enter checks out; Esc closes.
pub(super) fn branch_picker_handle_key(key: KeyEvent) -> ModalOutcome {
    match list_picker_key(key) {
        ListPickerKey::Up => ModalOutcome::Emit(Action::MoveGitBranchPickerCursor(-1)),
        ListPickerKey::Down => ModalOutcome::Emit(Action::MoveGitBranchPickerCursor(1)),
        ListPickerKey::Cancel => ModalOutcome::Emit(Action::CloseGitBranchPicker),
        ListPickerKey::Confirm => ModalOutcome::Emit(Action::ConfirmGitBranchPicker),
        ListPickerKey::Other => ModalOutcome::Continue,
    }
}

/// `y` / `Enter` → confirm push -u; `n` / `Esc` / `Ctrl-C` → cancel.
pub(super) fn set_upstream_confirm_handle_key(key: KeyEvent) -> ModalOutcome {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) => ModalOutcome::Emit(Action::GitCancelSetUpstream),
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            ModalOutcome::Emit(Action::GitCancelSetUpstream)
        }
        (KeyModifiers::NONE, KeyCode::Char('n')) | (KeyModifiers::NONE, KeyCode::Char('N')) => {
            ModalOutcome::Emit(Action::GitCancelSetUpstream)
        }
        (KeyModifiers::NONE, KeyCode::Char('y'))
        | (KeyModifiers::NONE, KeyCode::Char('Y'))
        | (_, KeyCode::Enter) => ModalOutcome::Emit(Action::GitConfirmSetUpstream),
        _ => ModalOutcome::Continue,
    }
}
