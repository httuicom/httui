//! Free helper functions used by the `App` impl + event loop.
//!
//! Mechanically extracted from `app.rs` (tui-v2 vertical 1, fase 2
//! p1-helpers) — pure code move, no behavior change. Bumped to
//! `pub(crate)` (were module-private free fns) and re-exported from
//! `app/mod.rs` so the existing intra-`app` call sites keep resolving.

use sqlx::SqlitePool;

use crate::pane::{Pane, PaneNode, TabState};

pub(crate) fn tab_has_dirty(tab: &TabState) -> bool {
    let mut dirty = false;
    for_each_leaf(&tab.root, &mut |pane| {
        if pane.document.as_ref().is_some_and(|d| d.is_dirty()) {
            dirty = true;
        }
    });
    dirty
}

pub(crate) fn for_each_leaf(node: &PaneNode, f: &mut impl FnMut(&Pane)) {
    match node {
        PaneNode::Leaf(p) => f(p),
        PaneNode::Split { first, second, .. } => {
            for_each_leaf(first, f);
            for_each_leaf(second, f);
        }
    }
}

pub(crate) fn for_each_leaf_mut(node: &mut PaneNode, f: &mut impl FnMut(&mut Pane)) {
    match node {
        PaneNode::Leaf(p) => f(p),
        PaneNode::Split { first, second, .. } => {
            for_each_leaf_mut(first, f);
            for_each_leaf_mut(second, f);
        }
    }
}

/// Snapshot the connection table into a `id → name` map so renderers
/// can stay sync. Falls back to an empty map on any error — the worst
/// case is footers showing the raw `connection=…` value from the fence.
pub(crate) fn load_connection_names(
    pool: &SqlitePool,
) -> std::collections::HashMap<String, String> {
    use httui_core::db::connections::list_connections;
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(list_connections(pool))
    });
    result
        .ok()
        .map(|conns| conns.into_iter().map(|c| (c.id, c.name)).collect())
        .unwrap_or_default()
}

pub(crate) fn file_name(p: &std::path::Path) -> String {
    p.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}
