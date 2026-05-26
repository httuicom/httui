//! Editor render pipeline: layout → clip to viewport → dispatch
//! per-segment renderer → cursor → status bar.

mod anchor;
mod block_history;
mod block_template_picker;
mod blocks;
mod completion_popup;
mod connection_delete_confirm;
mod connection_form;
mod connection_picker;
mod connections_page;
mod content_search;
mod cursor;
mod db_confirm_run;
mod db_export_picker;
pub mod db_row_detail;
mod db_settings_modal;
mod document;
mod environment_picker;
/// V4 P5: clone-env form renderer (extraído de envs_page).
mod envs_clone;
mod envs_page;
mod fence_edit;
mod git_branch_picker;
mod git_log_page;
mod git_panel;
mod git_set_upstream_confirm;
mod help;
pub mod http_response_detail;
mod overlay;
pub(crate) mod palette;
mod pane_tree;
mod prose;
mod quickopen;
mod render_root;
mod sql_highlight;
mod status;
mod tab_picker;
mod tabs;
mod tree;
/// V4 P7: "Used in N" panel pra var selecionada na EnvsPage.
mod var_uses_panel;
pub(crate) mod vault_clone_form;
pub(crate) mod vault_create_form;
mod vault_missing_secrets;
pub(crate) mod vault_open_picker;
mod vault_picker;

pub use render_root::render;

pub(crate) use anchor::BlockAnchor;
pub(crate) use document::{render_document, render_document_no_cursor};
pub(crate) use overlay::VisualOverlay;
pub(crate) use pane_tree::{render_empty_state_inline, render_pane_tree};
