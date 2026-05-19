//! Facade for the input dispatcher.
//!
//! The top-level `dispatch` router, the exhaustive `apply_action`
//! `Action` interpreter, and their `mod tests` moved to
//! `crate::input::dispatch` (tui-v2 vertical 1, fase 1
//! p6-router / p6-tests). This module is now a minimal re-export so
//! the existing `crate::vim::dispatch::*` call sites
//! (`app.rs::handle_key`, `crate::input::apply::replay`,
//! `vim::mod::dispatch`) keep resolving unchanged.

pub use crate::input::dispatch::dispatch;
// `apply_action` is `pub(crate)` — the `replay` helpers reach it via
// `crate::vim::dispatch::apply_action`.
pub(crate) use crate::input::dispatch::apply_action;
