//! In-memory markdown note representation for the TUI.
//!
//! A [`Document`] is a flat sequence of typed [`Segment`]s — prose
//! runs in [`ropey::Rope`]s, executable blocks as [`BlockNode`]s.
//! Parsing (`from_markdown`) and serializing (`to_markdown`) round-trip
//! through `httui_core::blocks`.
//!
//! Mutation APIs (insert / delete / undo) arrive in later rounds of
//! the current surface covers load, inspect, serialize.

// Some helpers (`is_e2e`, `BlockId::find_*`, …) are kept available but
// not yet consumed; epic 19/21 will pick them up.
#![allow(dead_code)]

pub mod block;
pub mod cursor;
pub mod document;
pub mod layout;
pub mod segment;

pub use cursor::Cursor;
pub use document::Document;
pub use segment::Segment;
