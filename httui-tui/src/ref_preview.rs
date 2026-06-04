//! `{{ref}}` hover-preview infrastructure for the BLOCKS + DOC views.
//!
//! When the user presses `K` (vim NORMAL) or `Alt+K` (standard) with
//! the cursor sitting INSIDE a complete `{{...}}` reference, this
//! module:
//!
//! 1. Locates the surrounding `{{...}}` span in the active text.
//! 2. Classifies it as an environment variable, a block reference,
//!    or unresolved — using the same rules
//!    `httui_core::references` uses at execution time.
//! 3. Returns a `RefPreviewState` the caller stuffs into
//!    [`crate::modal::Modal::RefPreview`].
//!
//! The detector is its own function so the renderer (which only sees
//! the state, not the document) can test against a stable contract.

use std::collections::HashMap;

use crate::app::{App, StatusKind};
use crate::buffer::{Cursor, Segment};
use crate::commands::db::load_active_env_vars;
use crate::modal::Modal;

/// What got resolved when we looked up the ref. The string fields are
/// already truncated by [`PREVIEW_VALUE_CHAR_LIMIT`] so the renderer
/// can paint them verbatim without worrying about screen overflow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefPreviewState {
    /// The ref body between the braces — e.g. `"login.response.id"` or
    /// `"BASE_URL"`. Stored without the `{{` / `}}` so the renderer
    /// chooses whether to re-add them.
    pub name: String,
    /// Where the value came from. Drives the header line in the popup
    /// (`from env: <env>` vs `from block: <alias>`).
    pub source: RefSource,
    /// Resolved value, truncated at `PREVIEW_VALUE_CHAR_LIMIT` chars
    /// (with a trailing `…` when chopped). Empty string for unresolved
    /// refs — the source field still tells the user *why* it's empty.
    pub value: String,
}

/// Where the resolved value came from. Drives the popup's header line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefSource {
    /// Environment variable resolved from the active env. Carries the
    /// active env name so the popup can show `from env: staging`.
    /// `None` when the active env couldn't be read.
    Env(Option<String>),
    /// Block alias found in the segments above the current one. The
    /// `cached` flag mirrors the autocomplete detail string so the
    /// user understands why an empty `value` happened.
    Block { alias: String, cached: bool },
    /// The ref name matched neither an env var nor a known block.
    Unknown,
}

/// Truncation limit for the popup value — long JSON payloads would
/// blow past the screen otherwise. Picked to fit two visual lines
/// comfortably on an 80-col terminal with no soft wrap.
const PREVIEW_VALUE_CHAR_LIMIT: usize = 120;

/// Find the `{{...}}` whose body span contains `byte_offset`. Returns
/// the ref body (no braces) when the cursor is INSIDE a complete ref,
/// `None` otherwise.
///
/// The cursor is considered "inside" when its byte offset lies in
/// `[open_brace_end, close_brace_start)` — i.e. anywhere between the
/// `{{` and the matching `}}`, exclusive of the closing braces. This
/// matches the user's intuition: the K chord fires while the caret is
/// *on* the ref name, not after it.
pub fn ref_under_cursor(text: &str, byte_offset: usize) -> Option<String> {
    let mut start = 0;
    while let Some(open_rel) = text[start..].find("{{") {
        let body_start = start + open_rel + 2;
        let Some(close_rel) = text[body_start..].find("}}") else {
            return None;
        };
        let body_end = body_start + close_rel;
        if byte_offset >= start + open_rel && byte_offset < body_end + 2 {
            let body = text[body_start..body_end].trim();
            if body.is_empty() {
                return None;
            }
            return Some(body.to_string());
        }
        start = body_end + 2;
    }
    None
}

/// Resolve a ref against the segments above the current block + the
/// active env vars. Mirrors `httui_core::references::resolve_string`
/// but emits a `RefPreviewState` instead of a substituted string.
///
/// `segments_above` MUST be the executable blocks preceding the one
/// the cursor is in (DAG-by-construction in the editor). `active_env`
/// is the active environment's name when known — only used to populate
/// the popup header.
pub fn resolve_ref(
    name: &str,
    segments_above: &[Segment],
    env_vars: &HashMap<String, String>,
    active_env: Option<&str>,
) -> RefPreviewState {
    if httui_core::references::is_block_reference(name) {
        resolve_block(name, segments_above, env_vars)
    } else {
        resolve_env(name, env_vars, active_env)
    }
}

fn resolve_env(
    name: &str,
    env_vars: &HashMap<String, String>,
    active_env: Option<&str>,
) -> RefPreviewState {
    if let Some(value) = env_vars.get(name) {
        RefPreviewState {
            name: name.to_string(),
            source: RefSource::Env(active_env.map(str::to_string)),
            value: truncate_for_preview(value),
        }
    } else {
        RefPreviewState {
            name: name.to_string(),
            source: RefSource::Unknown,
            value: String::new(),
        }
    }
}

fn resolve_block(
    name: &str,
    segments_above: &[Segment],
    env_vars: &HashMap<String, String>,
) -> RefPreviewState {
    let Some(alias) = httui_core::references::extract_alias(name) else {
        return RefPreviewState {
            name: name.to_string(),
            source: RefSource::Unknown,
            value: String::new(),
        };
    };
    // Look up the block by alias in the segments above the cursor
    // (forward find — matches the runtime resolver in
    // `commands::db::resolve_one_ref`, which picks the FIRST block
    // with the alias). No match → unknown ref, popup says so.
    let Some(block) = segments_above
        .iter()
        .filter_map(|s| match s {
            Segment::Block(b) => Some(b),
            _ => None,
        })
        .find(|b| b.alias.as_deref() == Some(alias))
    else {
        return RefPreviewState {
            name: name.to_string(),
            source: RefSource::Unknown,
            value: String::new(),
        };
    };
    let cached = block.cached_result.is_some();
    // Delegate to the runtime resolver so the popup sees `{{alias.
    // response.body.…}}`, the DB multi-result shim, and the env-var
    // fallback exactly the way `r` does at execution time. Errors
    // ("path not found", "alias not above", …) become an empty value
    // — the source chip + the `cached` flag still tell the user
    // why nothing landed.
    let value = match crate::commands::db::resolve_one_ref(
        segments_above,
        segments_above.len(),
        name,
        env_vars,
    ) {
        Ok(serde_json::Value::String(s)) => s,
        Ok(other) => other.to_string(),
        Err(_) => String::new(),
    };
    RefPreviewState {
        name: name.to_string(),
        source: RefSource::Block {
            alias: alias.to_string(),
            cached,
        },
        value: truncate_for_preview(&value),
    }
}

fn truncate_for_preview(s: &str) -> String {
    if s.chars().count() <= PREVIEW_VALUE_CHAR_LIMIT {
        return s.to_string();
    }
    let head: String = s.chars().take(PREVIEW_VALUE_CHAR_LIMIT).collect();
    format!("{head}…")
}

/// `Action::ShowRefPreview` applier. Reads the active document
/// (BLOCKS EDIT redirects to its field sub-doc automatically) plus
/// the cursor, locates the `{{ref}}` under the cursor and opens the
/// preview modal. Status-bar feedback when no ref is found so the
/// chord doesn't feel silently dropped.
pub fn show_ref_preview(app: &mut App) {
    let (segment_idx, offset) = match app.document().map(|d| d.cursor()) {
        Some(Cursor::InProse { segment_idx, offset }) => (segment_idx, offset),
        Some(Cursor::InBlock { segment_idx, offset }) => (segment_idx, offset),
        _ => {
            app.set_status(StatusKind::Info, "No {{ref}} under cursor");
            return;
        }
    };
    let Some(text) = app
        .document()
        .and_then(|d| d.segments().get(segment_idx).map(segment_text))
    else {
        return;
    };
    let Some(name) = ref_under_cursor(&text, offset) else {
        app.set_status(StatusKind::Info, "No {{ref}} under cursor");
        return;
    };
    // Block resolution needs the REAL file document — the active
    // doc (the field sub-Document in BLOCKS EDIT) has zero
    // `Segment::Block` entries by construction. `pane.document` is
    // the file the focused block lives in.
    let real_seg_idx = focused_block_segment_idx(app).unwrap_or(segment_idx);
    let mut segments_above: Vec<Segment> = app
        .active_pane()
        .and_then(|p| p.document.as_ref())
        .map(|doc| doc.segments().iter().take(real_seg_idx).cloned().collect())
        .unwrap_or_default();
    // Re-hydrate from SQLite so the latest cached_result lands in —
    // a sibling pane (or this pane's last run) may have refreshed
    // results since the document was loaded from disk. Without this
    // the popup would show "no result" right after a successful run.
    let env_vars: HashMap<String, String> = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(load_active_env_vars(&app.environments_store))
    })
    .unwrap_or_default();
    if let Some(key) = pane_file_key(app) {
        crate::block_hydrate::hydrate_segments_blocking(
            app.pool_manager.app_pool(),
            &mut segments_above,
            &env_vars,
            &key,
        );
    }
    let active_env = app.active_env_name.clone();
    let state = resolve_ref(&name, &segments_above, &env_vars, active_env.as_deref());
    app.modal = Some(Modal::RefPreview(state));
}

/// Resolve the segment index in `pane.document` that corresponds to
/// the block the user is currently editing (BLOCKS EDIT) or the
/// segment the cursor is on (DOC view). Matches `completion::
/// blocks_edit` semantics so both popups see the same "what block am
/// I in" answer.
fn focused_block_segment_idx(app: &App) -> Option<usize> {
    if !matches!(app.view, crate::app::AppView::Blocks) {
        return None;
    }
    let pane = app.active_pane()?;
    pane.block_edit.as_ref()?;
    let sel = pane.block_selected?;
    let ws = app.blocks_workspace.as_ref()?;
    let file = ws.index.files.get(sel.file_idx)?;
    let meta = file.blocks.get(sel.block_idx)?;
    let block_type = meta.block_type.as_str();
    let alias = meta.alias.as_deref();
    let doc = pane.document.as_ref()?;
    doc.segments().iter().position(|s| match s {
        Segment::Block(b) => {
            b.block_type.as_str() == block_type && b.alias.as_deref() == alias
        }
        _ => false,
    })
}

/// The pane's `document_path` is the same `PathBuf` the runtime
/// hands to `hydrate_segments_blocking` and the cache writer. It's
/// relative in DOC view (set by `open_in_new_tab`) and absolute in
/// BLOCKS EDIT (set by `enter_edit::load_and_hydrate`); the SQLite
/// `block_results.file_path` column mirrors that exactly, so we hand
/// it through untouched.
fn pane_file_key(app: &App) -> Option<std::path::PathBuf> {
    app.active_pane()?.document_path.clone()
}

fn segment_text(seg: &Segment) -> String {
    match seg {
        Segment::Prose(rope) => rope.to_string(),
        Segment::Block(block) => block.raw.to_string(),
    }
}

#[cfg(test)]
mod applier_tests {
    use super::*;
    use crate::app::{App, AppView, BlockIndex, BlocksWorkspace};
    use crate::buffer::block::{BlockId, BlockNode, ExecutionState};
    use crate::buffer::{Document, Segment};
    use crate::config::Config;
    use crate::modal::Modal;
    use crate::pane::Pane;
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Build an `App` rooted at a fresh vault containing `note.md`
    /// (caller supplies the body) plus an isolated SQLite pool. The
    /// initial active leaf carries that doc with its relative path,
    /// mirroring the `open_in_new_tab` flow.
    async fn app_with_note(body: &str) -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        let rel = PathBuf::from("note.md");
        std::fs::write(vault.path().join(&rel), body).unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let mut app = App::new(Config::default(), resolved, pool);
        let doc = Document::from_markdown(body).unwrap();
        if let Some(leaf) = app.active_pane_mut() {
            *leaf = Pane::new(doc, rel);
        }
        (app, data, vault)
    }

    fn cursor_inside_first_ref(app: &mut App) {
        // Park the cursor between the `{{` and `}}` of the first ref
        // we find. Lets `show_ref_preview` enter the resolver path
        // without depending on visual layout maths.
        let body = app
            .document()
            .and_then(|d| d.segments().first().map(segment_text))
            .unwrap_or_default();
        let open = body.find("{{").expect("body has a ref");
        let close = body[open..].find("}}").expect("close found") + open;
        let mid = (open + close + 2) / 2;
        if let Some(doc) = app.tabs.active_document_mut() {
            doc.set_cursor(crate::buffer::Cursor::InProse {
                segment_idx: 0,
                offset: mid,
            });
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn show_ref_preview_opens_unresolved_modal_for_unknown_env_var() {
        let (mut app, _d, _v) = app_with_note("Token: {{NO_SUCH_VAR}}\n").await;
        cursor_inside_first_ref(&mut app);
        show_ref_preview(&mut app);
        match app.modal {
            Some(Modal::RefPreview(state)) => {
                assert_eq!(state.name, "NO_SUCH_VAR");
                assert!(matches!(state.source, RefSource::Unknown));
                assert!(state.value.is_empty());
            }
            _ => panic!("expected RefPreview modal"),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn show_ref_preview_skips_when_cursor_not_on_a_ref() {
        let (mut app, _d, _v) = app_with_note("just prose, no refs\n").await;
        // Cursor at offset 0 — definitely not inside any `{{`.
        if let Some(doc) = app.tabs.active_document_mut() {
            doc.set_cursor(crate::buffer::Cursor::InProse {
                segment_idx: 0,
                offset: 0,
            });
        }
        show_ref_preview(&mut app);
        assert!(app.modal.is_none(), "no ref under cursor → no modal");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn focused_block_segment_idx_returns_none_outside_blocks_view() {
        let (mut app, _d, _v) = app_with_note("```http alias=a\nGET https://x\n```\n").await;
        // DOC view → helper bails.
        assert!(matches!(app.view, AppView::Doc));
        assert!(focused_block_segment_idx(&app).is_none());
        // BLOCKS view but no block_edit → also None.
        app.view = AppView::Blocks;
        app.blocks_workspace = Some(BlocksWorkspace::new(BlockIndex::build(&app.vault_path)));
        assert!(focused_block_segment_idx(&app).is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn pane_file_key_mirrors_document_path() {
        let (app, _d, _v) = app_with_note("hi\n").await;
        let key = pane_file_key(&app).expect("pane has path");
        assert_eq!(key, PathBuf::from("note.md"));
    }

    #[test]
    fn segment_text_returns_prose_string() {
        let seg = Segment::Prose(ropey::Rope::from_str("hello world"));
        assert_eq!(segment_text(&seg), "hello world");
    }

    #[test]
    fn segment_text_returns_block_raw() {
        let seg = Segment::Block(BlockNode {
            id: BlockId(0),
            raw: ropey::Rope::from_str("```http\nGET /\n```"),
            block_type: "http".into(),
            alias: None,
            display_mode: None,
            params: serde_json::json!({}),
            state: ExecutionState::Idle,
            cached_result: None,
        });
        assert!(segment_text(&seg).contains("GET /"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::block::{BlockId, BlockNode, ExecutionState};
    use crate::buffer::Segment;
    use ropey::Rope;
    use serde_json::json;

    fn make_block(alias: &str, cached: Option<serde_json::Value>) -> Segment {
        Segment::Block(BlockNode {
            id: BlockId(0),
            raw: Rope::new(),
            block_type: "http".to_string(),
            alias: Some(alias.to_string()),
            display_mode: None,
            params: json!({}),
            state: ExecutionState::Idle,
            cached_result: cached,
        })
    }

    fn block_with_cached(alias: &str, value: serde_json::Value) -> Segment {
        make_block(alias, Some(value))
    }

    #[test]
    fn ref_under_cursor_finds_env_var_with_cursor_inside_body() {
        // text:        Authorization: {{TOKEN}}
        // offsets:     0123456789012345678901234567890
        let text = "Authorization: {{TOKEN}}";
        // Cursor sits on `K` of TOKEN — should resolve.
        let token_k = text.find('K').unwrap();
        assert_eq!(
            ref_under_cursor(text, token_k),
            Some("TOKEN".to_string())
        );
    }

    #[test]
    fn ref_under_cursor_returns_none_outside_braces() {
        let text = "Authorization: {{TOKEN}} done";
        // Cursor on the trailing space outside the braces.
        let after = text.find(" done").unwrap();
        assert!(ref_under_cursor(text, after).is_none());
    }

    #[test]
    fn ref_under_cursor_handles_block_ref_with_dots() {
        let text = "Bearer {{login.response.token}}";
        let dot = text.find("response").unwrap();
        assert_eq!(
            ref_under_cursor(text, dot),
            Some("login.response.token".to_string())
        );
    }

    #[test]
    fn ref_under_cursor_returns_none_for_empty_braces() {
        let text = "{{}}";
        assert!(ref_under_cursor(text, 2).is_none());
    }

    #[test]
    fn ref_under_cursor_picks_the_enclosing_ref_when_multiple() {
        let text = "{{A}} and {{B}} and {{C}}";
        let b_offset = text.find('B').unwrap();
        assert_eq!(ref_under_cursor(text, b_offset), Some("B".to_string()));
        let c_offset = text.find('C').unwrap();
        assert_eq!(ref_under_cursor(text, c_offset), Some("C".to_string()));
    }

    #[test]
    fn ref_under_cursor_handles_unclosed_brace() {
        // `{{TOKEN` without `}}` — bail out, don't loop forever.
        assert!(ref_under_cursor("Bearer {{TOKEN", 9).is_none());
    }

    #[test]
    fn resolve_ref_returns_env_source_when_var_known() {
        let mut env = HashMap::new();
        env.insert("BASE_URL".to_string(), "https://api.x.com".to_string());
        let state = resolve_ref("BASE_URL", &[], &env, Some("staging"));
        assert_eq!(state.name, "BASE_URL");
        assert_eq!(state.value, "https://api.x.com");
        assert_eq!(state.source, RefSource::Env(Some("staging".into())));
    }

    #[test]
    fn resolve_ref_returns_unknown_when_env_var_missing() {
        let state = resolve_ref("MISSING", &[], &HashMap::new(), Some("staging"));
        assert_eq!(state.source, RefSource::Unknown);
        assert!(state.value.is_empty());
    }

    #[test]
    fn resolve_ref_returns_block_source_with_cached_value() {
        // Cached shape mirrors what HTTP/DB executors actually write:
        // top-level keys (`body`, `status`, …) WITHOUT a `response`
        // wrapper. The desktop-compat `{{alias.response.body.…}}`
        // syntax strips that literal `response` before navigating, so
        // both `{{login.body.token}}` and `{{login.response.body.
        // token}}` end up at `cached.body.token`.
        let segments = vec![block_with_cached(
            "login",
            json!({"body": {"token": "jwt-abc"}}),
        )];
        let state = resolve_ref(
            "login.response.body.token",
            &segments,
            &HashMap::new(),
            None,
        );
        assert_eq!(
            state.source,
            RefSource::Block {
                alias: "login".into(),
                cached: true,
            },
        );
        assert_eq!(state.value, "jwt-abc");
    }

    #[test]
    fn resolve_ref_block_alias_known_but_no_cached_result() {
        let segments = vec![make_block("login", None)];
        let state = resolve_ref("login.response.token", &segments, &HashMap::new(), None);
        assert_eq!(
            state.source,
            RefSource::Block {
                alias: "login".into(),
                cached: false,
            },
        );
        assert!(state.value.is_empty(), "no cached → no value");
    }

    #[test]
    fn resolve_ref_block_alias_unknown_returns_unknown() {
        let state = resolve_ref("noalias.response", &[], &HashMap::new(), None);
        assert_eq!(state.source, RefSource::Unknown);
    }

    #[test]
    fn resolve_ref_truncates_long_values() {
        let long = "x".repeat(PREVIEW_VALUE_CHAR_LIMIT + 50);
        let mut env = HashMap::new();
        env.insert("BIG".to_string(), long.clone());
        let state = resolve_ref("BIG", &[], &env, None);
        let count = state.value.chars().count();
        assert_eq!(count, PREVIEW_VALUE_CHAR_LIMIT + 1, "limit + ellipsis");
        assert!(state.value.ends_with('…'));
    }

    #[test]
    fn resolve_ref_picks_the_first_matching_alias() {
        // Two blocks with the same alias — runtime semantics (in
        // `commands::db::resolve_one_ref`) pick the FIRST match
        // walking the doc top-to-bottom. The popup mirrors that so
        // both surfaces agree on which value the executor would
        // actually substitute.
        let segments = vec![
            block_with_cached("auth", json!({"body": {"token": "first"}})),
            block_with_cached("auth", json!({"body": {"token": "second"}})),
        ];
        let state = resolve_ref(
            "auth.response.body.token",
            &segments,
            &HashMap::new(),
            None,
        );
        assert_eq!(state.value, "first");
    }
}
