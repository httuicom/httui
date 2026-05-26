//! Transient status-bar footer message.
//!
//! Mechanically extracted from `app.rs` (tui-v2 vertical 1, fase 2
//! p1-status) — pure code move, no behavior change. Re-exported from
//! `app/mod.rs` so `crate::app::{StatusKind, StatusMessage}` keep
//! resolving.

/// Severity hint for [`StatusMessage`]; drives the status-bar styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Info,
    Error,
}

/// Transient footer message — shown until the next keystroke replaces it.
/// Set by ex commands (`:w wrote …`, `:q error …`).
#[derive(Debug, Clone)]
pub struct StatusMessage {
    pub text: String,
    pub kind: StatusKind,
}
