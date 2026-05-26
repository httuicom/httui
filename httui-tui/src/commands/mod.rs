//! Domain commands — the actual *operations* the user invokes (run
//! a DB query, EXPLAIN one, export results, etc.). The vim layer
//! (`vim::dispatch`, `vim::ex`) only does key/mode → action routing
//! and then delegates here.
//!
//! Why this split: `vim::dispatch` was accumulating DB integration
//! logic (cache hashing, mutation detection, schema lookups, EXPLAIN
//! wrapping, the full execution flow) that has nothing to do with
//! vim. Extracting it lets the vim layer stay thin and the same
//! operations be invoked from other surfaces — keymaps, future
//! menus, MCP — without having to fish them out of dispatch.
//!
//! New surfaces should be exposed as **keymaps** (`KeyChord` in
//! `vim/keybindings.rs` + `Action` variant + `parse_normal` arm),
//! never as ex commands `:foo` — `vim/ex.rs` stays reserved for
//! the classic vim verbs (`:w`, `:q`, `:e`, `:noh`).

pub mod db;
pub mod git;
pub mod http;
pub mod refs;
pub mod search;
