//! Free helper functions used by the `App` impl + event loop.
//!
//! Mechanically extracted from `app.rs` (tui-v2 vertical 1, fase 2
//! p1-helpers) — pure code move, no behavior change. Bumped to
//! `pub(crate)` (were module-private free fns) and re-exported from
//! `app/mod.rs` so the existing intra-`app` call sites keep resolving.

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

/// Snapshot the vault's `connections.toml` into a `name → name` map
/// (V3 reordering 2026-05-23: vault TOML is now the source of truth;
/// the SQL `connections` table is no longer read here). The map shape
/// is preserved (`HashMap<String, String>`) so call-sites that lookup
/// by the block's `connection=` param keep working unchanged — TOML
/// uses the name as the key, so `id == name`. Falls back to an empty
/// map on any error.
pub(crate) fn load_connection_names(
    store: &httui_core::vault_config::ConnectionsStore,
) -> std::collections::HashMap<String, String> {
    tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(store.list_public()))
        .ok()
        .map(|conns| {
            conns
                .into_iter()
                .map(|c| (c.name.clone(), c.name))
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn file_name(p: &std::path::Path) -> String {
    p.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Document;
    use crate::pane::{Pane, SplitDir};
    use std::path::{Path, PathBuf};

    fn clean_pane(name: &str) -> Pane {
        Pane::new(
            Document::from_markdown("hello world\n").unwrap(),
            PathBuf::from(name),
        )
    }

    fn dirty_pane(name: &str) -> Pane {
        let mut doc = Document::from_markdown("hello world\n").unwrap();
        doc.mark_dirty();
        Pane::new(doc, PathBuf::from(name))
    }

    fn no_doc_pane(name: &str) -> Pane {
        Pane {
            document: None,
            document_path: Some(PathBuf::from(name)),
            viewport_top: 0,
            viewport_height: 0,
            block_selected: None,
            block_region: 0,
        }
    }

    fn split(first: PaneNode, second: PaneNode) -> PaneNode {
        PaneNode::Split {
            direction: SplitDir::Vertical,
            ratio: 0.5,
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    #[test]
    fn for_each_leaf_visits_every_leaf_in_a_split_tree() {
        // Tree: Split( Split(a, b), c ) — three leaves.
        let root = split(
            split(
                PaneNode::Leaf(clean_pane("a.md")),
                PaneNode::Leaf(clean_pane("b.md")),
            ),
            PaneNode::Leaf(clean_pane("c.md")),
        );
        let mut seen: Vec<String> = Vec::new();
        for_each_leaf(&root, &mut |p| {
            seen.push(p.document_path.as_ref().unwrap().display().to_string());
        });
        assert_eq!(seen, vec!["a.md", "b.md", "c.md"]);
    }

    #[test]
    fn for_each_leaf_handles_a_single_leaf_root() {
        let root = PaneNode::Leaf(clean_pane("solo.md"));
        let mut count = 0;
        for_each_leaf(&root, &mut |_| count += 1);
        assert_eq!(count, 1);
    }

    #[test]
    fn for_each_leaf_mut_can_mutate_every_leaf() {
        let mut root = split(
            PaneNode::Leaf(clean_pane("a.md")),
            PaneNode::Leaf(clean_pane("b.md")),
        );
        for_each_leaf_mut(&mut root, &mut |p| {
            p.viewport_top = 42;
        });
        let mut tops: Vec<u16> = Vec::new();
        for_each_leaf(&root, &mut |p| tops.push(p.viewport_top));
        assert_eq!(tops, vec![42, 42]);
    }

    #[test]
    fn for_each_leaf_mut_handles_a_single_leaf_root() {
        let mut root = PaneNode::Leaf(clean_pane("solo.md"));
        for_each_leaf_mut(&mut root, &mut |p| p.viewport_height = 7);
        let mut h = 0;
        for_each_leaf(&root, &mut |p| h = p.viewport_height);
        assert_eq!(h, 7);
    }

    #[test]
    fn tab_has_dirty_false_when_all_panes_clean_or_empty() {
        let tab = TabState {
            root: split(
                PaneNode::Leaf(clean_pane("a.md")),
                PaneNode::Leaf(no_doc_pane("b.md")),
            ),
            focused: Vec::new(),
        };
        assert!(!tab_has_dirty(&tab));
    }

    #[test]
    fn tab_has_dirty_true_when_any_leaf_is_dirty() {
        let tab = TabState {
            root: split(
                PaneNode::Leaf(clean_pane("a.md")),
                split(
                    PaneNode::Leaf(no_doc_pane("b.md")),
                    PaneNode::Leaf(dirty_pane("c.md")),
                ),
            ),
            focused: Vec::new(),
        };
        assert!(tab_has_dirty(&tab));
    }

    #[test]
    fn tab_has_dirty_single_dirty_leaf() {
        let tab = TabState {
            root: PaneNode::Leaf(dirty_pane("only.md")),
            focused: Vec::new(),
        };
        assert!(tab_has_dirty(&tab));
    }

    #[test]
    fn file_name_returns_last_component() {
        assert_eq!(file_name(Path::new("/a/b/note.md")), "note.md");
        assert_eq!(file_name(Path::new("note.md")), "note.md");
    }

    #[test]
    fn file_name_falls_back_to_full_display_when_no_file_component() {
        // A root path has no `file_name()` — falls back to the
        // display form rather than panicking.
        let p = Path::new("/");
        assert_eq!(file_name(p), p.display().to_string());
    }

    // ───────────── load_connection_names ─────────────
    //
    // Exercises `block_in_place` + `Handle::current().block_on(...)`.
    // `block_in_place` panics on a current-thread runtime, so these
    // are `#[tokio::test(flavor = "multi_thread")]`.

    #[tokio::test(flavor = "multi_thread")]
    async fn load_connection_names_maps_name_to_name() {
        use httui_core::vault_config::{ConnectionsStore, CreateConnectionInput};
        use tempfile::TempDir;

        let vault = TempDir::new().unwrap();
        let store = ConnectionsStore::new(vault.path());
        store
            .create(CreateConnectionInput {
                name: "prod-db".into(),
                driver: "postgres".into(),
                host: Some("localhost".into()),
                port: Some(5432),
                database_name: Some("db".into()),
                username: Some("user".into()),
                password: None,
                ssl_mode: None,
                is_readonly: None,
                description: None,
            })
            .await
            .unwrap();

        let names = load_connection_names(&store);
        assert_eq!(names.get("prod-db").map(String::as_str), Some("prod-db"));
        assert_eq!(names.len(), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn load_connection_names_empty_when_no_connections() {
        use httui_core::vault_config::ConnectionsStore;
        use tempfile::TempDir;

        let vault = TempDir::new().unwrap();
        let store = ConnectionsStore::new(vault.path());
        // Fresh vault → no connections.toml → empty map (not an error).
        assert!(load_connection_names(&store).is_empty());
    }
}
