//! Cross-block dependency orchestration for the run flow.
//!
//! When a block references `{{A.response.x}}` and A hasn't run yet,
//! the resolver in `commands::db::resolve_one_ref` errors out with
//! `block ``A`` hasn't run yet`. This module turns that hard error
//! into auto-execution: before the target runs, we collect every
//! upstream block that still needs to execute (DAG order), enqueue
//! them on `app.run_chain`, and dispatch one at a time. Each
//! `handle_*_block_result` calls back here on success to advance the
//! queue; failures abort the chain (the failed block stays Error and
//! the user fixes it before retrying).
//!
//! Invariants:
//!   - Block refs only point to blocks ABOVE the citing block (DAG
//!     by construction; the parser rejects circular topologies).
//!     A direct cycle is impossible structurally, but the collector
//!     still tracks `in_progress` defensively so a future
//!     "below-allowed" tweak couldn't introduce silent infinite
//!     recursion.
//!   - Already-cached blocks (cached_result.is_some) are skipped —
//!     this is the dedup story for B + C both citing A: A runs once
//!     because the second collection sees its cached_result.
//!   - Max depth caps stack growth at 50 levels (mirrors the core
//!     runner's `MAX_DEPENDENCY_DEPTH`).

use crate::app::{App, StatusKind};
use crate::buffer::{Cursor, Segment};
use httui_core::references;
use std::collections::HashSet;

const MAX_DEPENDENCY_DEPTH: usize = 50;

/// Walk every `{{ref}}` in the target block's params and return the
/// upstream blocks that need to run before it. Output is in
/// execution order (deepest dep first, immediate dep last); the
/// caller appends the target onto the end before dispatching.
///
/// Skips deps whose `cached_result.is_some()` — they already have a
/// value the resolver can pick up.
pub fn collect_unrun_deps(
    segments: &[Segment],
    target_idx: usize,
) -> Result<Vec<usize>, String> {
    let mut out: Vec<usize> = Vec::new();
    let mut seen: HashSet<usize> = HashSet::new();
    let mut in_progress: HashSet<usize> = HashSet::new();
    walk_deps(segments, target_idx, &mut out, &mut seen, &mut in_progress, 0)?;
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
    if in_progress.contains(&idx) {
        // Above-only refs prevent this structurally, but a defensive
        // check keeps the error surface honest if the invariant ever
        // weakens.
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
            continue; // env var, not a block dep
        }
        let alias = match references::extract_alias(&placeholder) {
            Some(a) if !a.is_empty() => a,
            _ => continue,
        };
        // Find the matching block strictly above `idx`.
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
            // Unknown alias → let the existing resolver fail at run
            // time with the canonical message; we don't second-guess
            // it here.
            continue;
        };
        // Already cached? Skip — dedup case for B + C citing A.
        let dep_block = match segments.get(dep_idx) {
            Some(Segment::Block(b)) => b,
            _ => continue,
        };
        if dep_block.cached_result.is_some() || seen.contains(&dep_idx) {
            continue;
        }
        // Recurse into the dep first so its OWN unrun deps land
        // earlier in `out` (post-order topo).
        walk_deps(segments, dep_idx, out, seen, in_progress, depth + 1)?;
        if seen.insert(dep_idx) {
            out.push(dep_idx);
        }
    }
    in_progress.remove(&idx);
    Ok(())
}

/// Entry point for the user-facing "run this block" action. Replaces
/// the direct dispatch in `commands::db::apply_run_block` for the
/// happy path: collect deps, queue chain, dispatch first link.
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

/// Dispatch the head of `app.run_chain` to the right block executor.
/// Called by `start_run_chain` for the first link and by the
/// `handle_*_block_result` hooks for each subsequent link.
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
        crate::commands::db::run_db_block_inner(
            app, next_idx, /* force_unscoped = */ false, None,
            /* as_explain = */ false,
        );
    } else {
        app.run_chain.clear();
        app.set_status(
            StatusKind::Info,
            format!("`{block_type}` blocks aren't runnable yet"),
        );
        return;
    }

    // Stall guard. Both `apply_run_http_block` and `run_db_block_inner`
    // have early-return paths (validation errors: empty URL, no
    // connection, unresolved ref, etc.) that never spawn an async
    // task and never fire the `AppEvent` the chain depends on. When
    // that happens, `app.running_query` stays `None` AND the head of
    // `run_chain` is still `next_idx` (cache hits advance the head
    // synchronously, so they don't trigger this). Without this check
    // the chain freezes silently — the user has to mash the run key
    // to advance one link at a time.
    if app.run_chain.first() == Some(&next_idx) && app.running_query.is_none() {
        app.run_chain.clear();
    }
}

/// Hook called by `handle_*_block_result` after the outcome has been
/// folded into the block. On success, pop the chain head and
/// dispatch the next link. On failure / cancel, clear the chain so
/// the user can fix the failing dep before retrying.
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

/// Convenience entry for the user's `r` keypress / `RunBlock` action.
/// Picks the cursor's block and starts a chain rooted there. Returns
/// early with a hint when the cursor isn't on a block.
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
        assert_eq!(a_count, 1, "A must run once even for diamond; deps={deps:?}");
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
}
