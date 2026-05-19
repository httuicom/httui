//! Input layer — keymap profiles (standard / vim) over a shared
//! `Action` dispatch.
//!
//! Home of the decoupled input architecture (tui-v2 vertical 1). This
//! module is being populated by a *mechanical* split of
//! `vim/parser.rs` and `vim/dispatch.rs`: every submodule is a pure
//! code move with no logic change, and the existing vim test suite is
//! the safety net (it must stay green without a single test edited).
//!
//! Plan: `docs-llm/tui-v2/vertical-01-input-model.md`. Until the split
//! lands, `vim::parser` / `vim::dispatch` remain thin re-export
//! facades so the ~16 existing call sites need no changes.

pub mod action;
pub mod types;
