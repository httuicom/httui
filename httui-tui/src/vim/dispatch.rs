//! Facade for the input dispatcher.
//!
//! The top-level `dispatch` router, the exhaustive `apply_action`
//! `Action` interpreter, and their `mod tests` moved to
//! `crate::input::dispatch` (tui-v2 vertical 1, fase 1
//! p6-router / p6-tests). This module is now a minimal re-export so
//! the surviving `crate::vim::dispatch::*` call site
//! (`crate::input::apply::replay`) keeps resolving unchanged.
//!
//! The `dispatch` fn re-export was dropped in tui-V1 / fase 2 p5:
//! `app::handle_key` no longer routes through the vim facade — it
//! goes through `crate::input::route::route`, which calls
//! `crate::input::dispatch::dispatch` directly for the Vim profile.
//! Only `apply_action` is still consumed via this facade.

// `apply_action` is `pub(crate)` — the `replay` helpers reach it via
// `crate::vim::dispatch::apply_action`.
pub(crate) use crate::input::dispatch::apply_action;
