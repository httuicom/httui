//! Editor render pipeline: layout → clip to viewport → dispatch
//! per-segment renderer → cursor → status bar.

mod anchor;
mod block_history;
mod block_template_picker;
mod blocks;
mod blocks_unsaved_prompt;
mod blocks_view;
mod completion_popup;
mod confirm_prompt;
mod connection_form;
mod connection_picker;
mod connections_page;
mod content_search;
mod cursor;
mod db_export_picker;
pub mod db_row_detail;
mod db_settings_modal;
mod document;
mod environment_picker;
/// Clone-env form renderer (extracted from `envs_page`).
mod envs_clone;
mod envs_page;
mod fence_edit;
mod git_branch_picker;
mod git_conflict_resolver;
mod git_log_page;
mod git_panel;
mod git_panel_form;
mod git_panel_history;
mod git_set_upstream_confirm;
mod help;
pub mod http_response_detail;
mod overlay;
pub(crate) mod palette;
mod pane_tree;
mod prose;
mod quickopen;
mod render_modals;
mod render_root;
/// Settings page (Keymaps / Theme / Editor).
mod settings_page;
mod sql_highlight;
mod status;
mod tab_picker;
mod tabs;
/// Runtime palette + named presets consumed by `palette.rs`.
pub mod theme;
mod tree;
/// "Used in N" panel for the var selected in EnvsPage.
mod var_uses_panel;
pub(crate) mod vault_clone_form;
pub(crate) mod vault_create_form;
mod vault_missing_secrets;
pub(crate) mod vault_open_picker;
mod vault_picker;

pub use render_root::render;

pub(crate) use anchor::BlockAnchor;
pub(crate) use document::{render_document, render_document_no_cursor};
pub(crate) use overlay::{overlay_visual_selection, VisualOverlay};
pub(crate) use status::running_chip_label;
pub(crate) use pane_tree::{render_empty_state_inline, render_pane_tree};
