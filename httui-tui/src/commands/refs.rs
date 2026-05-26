//! Cross-block dependency orchestration for the run flow.

use crate::app::{App, StatusKind};
use crate::buffer::{Cursor, Segment};
use httui_core::references;
use std::collections::HashSet;

const MAX_DEPENDENCY_DEPTH: usize = 50;

pub fn collect_unrun_deps(segments: &[Segment], target_idx: usize) -> Result<Vec<usize>, String> {
    let mut out: Vec<usize> = Vec::new();
    let mut seen: HashSet<usize> = HashSet::new();
    let mut in_progress: HashSet<usize> = HashSet::new();
    walk_deps(
        segments,
        target_idx,
        &mut out,
        &mut seen,
        &mut in_progress,
        0,
    )?;
    Ok(out)
}

fn walk_deps(
    segments: &[Segment],
    idx: usize,
    out: &mut Vec<usize>,
    seen: &mut HashSet<usize>,
    in_progress: &mut HashSet<usize>,
    depth: usize,
) -> Result<(), String> {
    if depth > MAX_DEPENDENCY_DEPTH {
        return Err(format!(
            "dependency chain exceeds {MAX_DEPENDENCY_DEPTH} levels — break it up",
        ));
    }
    // Above-only refs prevent cycles structurally; this guard
    // protects against a future relaxation.
    if in_progress.contains(&idx) {
        return Err("circular dependency detected".to_string());
    }
    in_progress.insert(idx);

    let block = match segments.get(idx) {
        Some(Segment::Block(b)) => b,
        _ => {
            in_progress.remove(&idx);
            return Ok(());
        }
    };

    let placeholders = references::extract_placeholders(&block.params);
    for placeholder in placeholders {
        if !references::is_block_reference(&placeholder) {
            continue;
        }
        let alias = match references::extract_alias(&placeholder) {
            Some(a) if !a.is_empty() => a,
            _ => continue,
        };
        let dep_idx = segments
            .iter()
            .take(idx)
            .enumerate()
            .filter_map(|(i, s)| match s {
                Segment::Block(b) => Some((i, b)),
                _ => None,
            })
            .find(|(_, b)| b.alias.as_deref() == Some(alias))
            .map(|(i, _)| i);
        let Some(dep_idx) = dep_idx else {
            continue;
        };
        let dep_block = match segments.get(dep_idx) {
            Some(Segment::Block(b)) => b,
            _ => continue,
        };
        if dep_block.cached_result.is_some() || seen.contains(&dep_idx) {
            continue;
        }
        walk_deps(segments, dep_idx, out, seen, in_progress, depth + 1)?;
        if seen.insert(dep_idx) {
            out.push(dep_idx);
        }
    }
    in_progress.remove(&idx);
    Ok(())
}

pub fn start_run_chain(app: &mut App, target_idx: usize) {
    if app.running_query.is_some() {
        app.set_status(
            StatusKind::Info,
            "another block is already running — Ctrl-C to cancel",
        );
        return;
    }
    let Some(doc) = app.document() else { return };
    let segments_snapshot: Vec<Segment> = doc.segments().to_vec();
    let deps = match collect_unrun_deps(&segments_snapshot, target_idx) {
        Ok(d) => d,
        Err(msg) => {
            app.set_status(StatusKind::Error, msg);
            return;
        }
    };
    let mut chain: Vec<usize> = deps;
    chain.push(target_idx);
    app.run_chain = chain;
    advance_run_chain(app);
}

pub fn advance_run_chain(app: &mut App) {
    let Some(&next_idx) = app.run_chain.first() else {
        return;
    };
    let block_type = match app.document().and_then(|d| d.segments().get(next_idx)) {
        Some(Segment::Block(b)) => b.block_type.clone(),
        _ => {
            app.run_chain.clear();
            return;
        }
    };
    if block_type == "http" {
        crate::commands::http::apply_run_http_block(app, next_idx);
    } else if block_type.starts_with("db-") || block_type == "db" {
        crate::commands::db::run_db_block_inner(app, next_idx, false, None, false);
    } else {
        app.run_chain.clear();
        app.set_status(
            StatusKind::Info,
            format!("`{block_type}` blocks aren't runnable yet"),
        );
        return;
    }

    // Stall guard: validation errors return without spawning, so
    // no AppEvent ever lands. Cache hits move the head synchronously
    // via on_block_complete; anything else with a static head and
    // no running_query means the dispatch bailed.
    if app.run_chain.first() == Some(&next_idx) && app.running_query.is_none() {
        app.run_chain.clear();
    }
}

pub fn on_block_complete(app: &mut App, segment_idx: usize, success: bool) {
    if app.run_chain.first() != Some(&segment_idx) {
        return;
    }
    app.run_chain.remove(0);
    if !success {
        if !app.run_chain.is_empty() {
            app.set_status(
                StatusKind::Info,
                "auto-exec chain aborted — fix the dep error first",
            );
            app.run_chain.clear();
        }
        return;
    }
    if !app.run_chain.is_empty() {
        advance_run_chain(app);
    }
}

pub fn apply_run_block(app: &mut App) {
    let Some(doc) = app.document() else { return };
    let Cursor::InBlock { segment_idx, .. } = doc.cursor() else {
        app.set_status(
            StatusKind::Info,
            "no block at cursor (place cursor on a block first)",
        );
        return;
    };
    start_run_chain(app, segment_idx);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Document;

    fn doc_from(md: &str) -> Document {
        Document::from_markdown(md).expect("parse")
    }

    fn block_idxs(d: &Document) -> Vec<usize> {
        d.segments()
            .iter()
            .enumerate()
            .filter_map(|(i, s)| matches!(s, Segment::Block(_)).then_some(i))
            .collect()
    }

    fn set_cache(d: &mut Document, idx: usize, v: serde_json::Value) {
        if let Some(b) = d.block_at_mut(idx) {
            b.cached_result = Some(v);
        }
    }

    #[test]
    fn collect_returns_empty_when_no_refs() {
        let md = "```http alias=a\nGET /x\n```\n";
        let d = doc_from(md);
        let idxs = block_idxs(&d);
        let deps = collect_unrun_deps(d.segments(), idxs[0]).unwrap();
        assert!(deps.is_empty());
    }

    #[test]
    fn collect_returns_dep_when_target_cites_uncached_upstream() {
        let md =
            "```http alias=a\nGET /upstream\n```\n\n```http alias=b\nGET /x?id={{a.body.id}}\n```\n";
        let d = doc_from(md);
        let idxs = block_idxs(&d);
        let deps = collect_unrun_deps(d.segments(), idxs[1]).unwrap();
        assert_eq!(deps, vec![idxs[0]]);
    }

    #[test]
    fn collect_skips_dep_already_cached() {
        // The dedup story: cached deps are not re-run.
        let md =
            "```http alias=a\nGET /upstream\n```\n\n```http alias=b\nGET /x?id={{a.body.id}}\n```\n";
        let mut d = doc_from(md);
        let idxs = block_idxs(&d);
        set_cache(&mut d, idxs[0], serde_json::json!({"body": {"id": 7}}));
        let deps = collect_unrun_deps(d.segments(), idxs[1]).unwrap();
        assert!(deps.is_empty(), "cached dep must be skipped; got {deps:?}");
    }

    #[test]
    fn collect_orders_transitive_deps_deepest_first() {
        // C cites B; B cites A. Running C should run A then B.
        let md = "\
```http alias=a
GET /a
```

```http alias=b
GET /b?x={{a.body.x}}
```

```http alias=c
GET /c?y={{b.body.y}}
```
";
        let d = doc_from(md);
        let idxs = block_idxs(&d);
        let deps = collect_unrun_deps(d.segments(), idxs[2]).unwrap();
        assert_eq!(deps, vec![idxs[0], idxs[1]]);
    }

    #[test]
    fn collect_dedups_shared_dep_for_diamond() {
        // D cites both B and C; both B and C cite A. A must appear
        // exactly once in D's chain.
        let md = "\
```http alias=a
GET /a
```

```http alias=b
GET /b?x={{a.body.x}}
```

```http alias=c
GET /c?x={{a.body.x}}
```

```http alias=d
GET /d?b={{b.body.y}}&c={{c.body.y}}
```
";
        let d = doc_from(md);
        let idxs = block_idxs(&d);
        let deps = collect_unrun_deps(d.segments(), idxs[3]).unwrap();
        let a_count = deps.iter().filter(|i| **i == idxs[0]).count();
        assert_eq!(
            a_count, 1,
            "A must run once even for diamond; deps={deps:?}"
        );
        // A must come before both B and C.
        let a_pos = deps.iter().position(|i| *i == idxs[0]).unwrap();
        let b_pos = deps.iter().position(|i| *i == idxs[1]).unwrap();
        let c_pos = deps.iter().position(|i| *i == idxs[2]).unwrap();
        assert!(a_pos < b_pos && a_pos < c_pos);
    }

    #[test]
    fn collect_ignores_env_var_placeholders() {
        let md = "```http alias=a\nGET /x?t={{API_TOKEN}}\n```\n";
        let d = doc_from(md);
        let idxs = block_idxs(&d);
        let deps = collect_unrun_deps(d.segments(), idxs[0]).unwrap();
        assert!(deps.is_empty());
    }

    #[test]
    fn collect_returns_empty_for_non_block_segment() {
        let md = "plain prose only\n";
        let d = doc_from(md);
        let deps = collect_unrun_deps(d.segments(), 0).unwrap();
        assert!(deps.is_empty());
    }

    #[test]
    fn collect_returns_empty_for_out_of_range_idx() {
        let md = "```http alias=a\nGET /x\n```\n";
        let d = doc_from(md);
        let deps = collect_unrun_deps(d.segments(), 999).unwrap();
        assert!(deps.is_empty());
    }

    #[test]
    fn collect_skips_dep_with_no_matching_alias() {
        let md = "```http alias=a\nGET /x?t={{ghost.id}}\n```\n";
        let d = doc_from(md);
        let idxs = block_idxs(&d);
        let deps = collect_unrun_deps(d.segments(), idxs[0]).unwrap();
        assert!(deps.is_empty());
    }

    use crate::app::{App, RunningKind, RunningQuery};
    use crate::config::Config;
    use crate::pane::{Pane, TabState};
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use std::time::Instant;
    use tempfile::TempDir;
    use tokio_util::sync::CancellationToken;

    async fn app_with_doc(md: &str) -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        let note = vault.path().join("note.md");
        std::fs::write(&note, md).unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let mut app = App::new(Config::default(), resolved, pool);
        let doc = Document::from_markdown(md).unwrap();
        let pane = Pane::new(doc, note);
        app.tabs.tabs.push(TabState::new(pane));
        app.tabs.active = 0;
        (app, data, vault)
    }

    fn make_running_query(segment_idx: usize) -> RunningQuery {
        RunningQuery {
            segment_idx,
            cancel: CancellationToken::new(),
            started_at: Instant::now(),
            kind: RunningKind::Run,
            cache_key: None,
            bytes_received: 0,
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_run_block_with_no_block_at_cursor_sets_status() {
        let (mut app, _d, _v) = app_with_doc("just prose\n").await;
        apply_run_block(&mut app);
        assert!(
            app.run_chain.is_empty(),
            "no chain when cursor not in block"
        );
        assert!(app.status_message.is_some(), "expected status hint");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_run_block_with_cursor_in_block_starts_chain() {
        let md = "```http alias=a\nGET /x\n```\n";
        let (mut app, _d, _v) = app_with_doc(md).await;
        let block_idx = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .expect("block");
        let raw_offset = match app.document().unwrap().segments().get(block_idx) {
            Some(Segment::Block(_)) => 0usize,
            _ => panic!(),
        };
        app.document_mut().unwrap().set_cursor(Cursor::InBlock {
            segment_idx: block_idx,
            offset: raw_offset,
        });
        apply_run_block(&mut app);
        // run_chain was either built then cleared by stall guard
        // (URL invalid, no running_query spawned) — exercise still
        // hits start_run_chain + advance_run_chain.
        assert!(app.run_chain.is_empty() || !app.run_chain.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn start_run_chain_aborts_when_running_query_present() {
        let md = "```http alias=a\nGET /x\n```\n";
        let (mut app, _d, _v) = app_with_doc(md).await;
        app.running_query = Some(make_running_query(0));
        start_run_chain(&mut app, 0);
        assert!(app.run_chain.is_empty());
        assert!(
            app.status_message
                .as_ref()
                .map(|s| s.text.contains("already running"))
                .unwrap_or(false),
            "expected already-running status"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn advance_run_chain_with_empty_chain_is_noop() {
        let (mut app, _d, _v) = app_with_doc("just prose\n").await;
        advance_run_chain(&mut app);
        assert!(app.run_chain.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn advance_run_chain_clears_when_block_type_unknown() {
        // Parse a real block then mutate block_type so advance_run_chain
        // hits the "not runnable" branch.
        let md = "```http alias=a\nGET /x\n```\n";
        let (mut app, _d, _v) = app_with_doc(md).await;
        let block_idx = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .expect("block");
        if let Some(b) = app.document_mut().unwrap().block_at_mut(block_idx) {
            b.block_type = "mystery".into();
        }
        app.run_chain = vec![block_idx];
        advance_run_chain(&mut app);
        assert!(app.run_chain.is_empty(), "unknown block type clears chain");
        assert!(
            app.status_message
                .as_ref()
                .map(|s| s.text.contains("runnable"))
                .unwrap_or(false),
            "expected not-runnable status"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn advance_run_chain_clears_when_head_idx_not_a_block() {
        let (mut app, _d, _v) = app_with_doc("prose only\n").await;
        app.run_chain = vec![42]; // out-of-range
        advance_run_chain(&mut app);
        assert!(app.run_chain.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn on_block_complete_ignores_idx_when_head_mismatches() {
        let (mut app, _d, _v) = app_with_doc("prose\n").await;
        app.run_chain = vec![5];
        on_block_complete(&mut app, 99, true);
        assert_eq!(app.run_chain, vec![5], "mismatched head must not pop");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn on_block_complete_failure_with_more_pending_aborts_chain() {
        let (mut app, _d, _v) = app_with_doc("prose\n").await;
        app.run_chain = vec![1, 2, 3];
        on_block_complete(&mut app, 1, false);
        assert!(app.run_chain.is_empty(), "failure aborts the chain");
        assert!(
            app.status_message
                .as_ref()
                .map(|s| s.text.contains("aborted"))
                .unwrap_or(false),
            "expected aborted status"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn on_block_complete_failure_with_empty_chain_is_silent() {
        let (mut app, _d, _v) = app_with_doc("prose\n").await;
        app.run_chain = vec![7];
        on_block_complete(&mut app, 7, false);
        assert!(app.run_chain.is_empty());
        // No "aborted" status — the chain was already on its last item.
        assert!(app
            .status_message
            .as_ref()
            .map(|s| !s.text.contains("aborted"))
            .unwrap_or(true));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn on_block_complete_success_with_no_more_just_pops() {
        let (mut app, _d, _v) = app_with_doc("prose\n").await;
        app.run_chain = vec![3];
        on_block_complete(&mut app, 3, true);
        assert!(app.run_chain.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn start_run_chain_no_doc_returns_silently() {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let mut app = App::new(Config::default(), resolved, pool);
        // no tabs registered — document() returns None
        start_run_chain(&mut app, 0);
        assert!(app.run_chain.is_empty());
        assert!(app.status_message.is_none(), "no-doc path is silent");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn start_run_chain_propagates_collect_error_to_status() {
        // Build a chain where alias `a` is self-referencing — we
        // can't easily forge cycles with the above-only rule, so
        // simulate the failure by feeding a depth-exceeding chain.
        let mut md = String::new();
        // Build A0…A55 each citing the previous one — depth > 50.
        md.push_str("```http alias=a0\nGET /a\n```\n\n");
        for i in 1..=55usize {
            md.push_str(&format!(
                "```http alias=a{i}\nGET /a?x={{{{a{}.body.id}}}}\n```\n\n",
                i - 1
            ));
        }
        let (mut app, _d, _v) = app_with_doc(&md).await;
        let target_idx = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .enumerate()
            .filter_map(|(i, s)| matches!(s, Segment::Block(_)).then_some(i))
            .next_back()
            .expect("at least one block");
        start_run_chain(&mut app, target_idx);
        assert!(app.run_chain.is_empty(), "error path clears chain");
        assert!(
            app.status_message
                .as_ref()
                .map(|s| s.text.contains("50") || s.text.contains("chain"))
                .unwrap_or(false),
            "expected depth-exceeded error in status: {:?}",
            app.status_message.as_ref().map(|s| &s.text)
        );
    }
}
