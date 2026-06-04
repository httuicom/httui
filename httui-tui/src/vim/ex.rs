//! Ex commands — the `:foo` family. Round 2 covers the bare minimum:
//! `:w`, `:q`, `:wq` (alias `:x`), `:q!`. Everything else returns
//! [`ExResult::Unknown`] so callers can surface a `not an editor command`
//! error in the status bar.
//!
//! Persistence side-effect: `:w` serializes the document via
//! [`crate::buffer::Document::to_markdown`] and writes it to
//! `app.document_path`. No path → error.

use std::path::PathBuf;

use crate::app::App;

/// Parsed ex command. `force` is the trailing `!`: `:q!` overrides the
/// dirty-buffer guard; `:e!` is the same idea for opening another file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExCmd {
    Write,
    Quit {
        force: bool,
    },
    WriteQuit,
    /// `:e <path>` / `:edit <path>` / `:e! <path>`.
    Edit {
        path: String,
        force: bool,
    },
    /// `:noh` / `:nohlsearch` — clear the active search highlight.
    NoHighlight,
    /// `:%s/pattern/replacement[/]` — global, literal substitution
    /// across the entire document. V1 is intentionally narrow:
    /// no regex, no per-line scope (`:s/...`), no flags. Trailing
    /// `/` (or `/g`) is accepted but treated as a no-op since the
    /// scope is always document-global. The substitution serializes
    /// the doc to markdown, replaces the literal substring, and
    /// re-parses — works uniformly across prose and block bodies
    /// so renaming an alias used in `{{alias.x}}` refs Just Works.
    Substitute {
        pattern: String,
        replacement: String,
    },
    /// `:N` — bare-number form: jump to line N (1-indexed). Vim
    /// convention; useful for navigating to a line cited in a
    /// stack trace or compiler output. Reuses the `Motion::GotoLine`
    /// machinery so the cursor lands the same way `<n>G` does.
    GotoLine(usize),
}

/// Outcome of an ex command. `Ok(msg)` carries a status string for the
/// footer; `Err(msg)` does the same but signals failure styling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExResult {
    Ok(String),
    Err(String),
    /// Quit was requested — caller is responsible for setting
    /// `should_quit = true` (after persisting any final state).
    Quit,
    /// Buffer was empty (just `:`<Enter>) — silently no-op.
    Empty,
    /// Did not match any known command.
    Unknown(String),
}

/// Parse the cmdline buffer (no leading `:`) into an [`ExCmd`].
pub fn parse(buf: &str) -> Result<ExCmd, ParseError> {
    let trimmed = buf.trim();
    if trimmed.is_empty() {
        return Err(ParseError::Empty);
    }

    // `:%s/pattern/replacement[/flags]` — handled before the
    // whitespace-split branch because the command lives glued to
    // its delimiter (`%s/…`), no whitespace separates them.
    if let Some(rest) = trimmed.strip_prefix("%s/") {
        return parse_substitute(rest);
    }

    // `:N` — bare positive integer is goto-line. Doesn't match the
    // whitespace-split branch below because there's no head/tail
    // boundary; check it before that path.
    if let Ok(n) = trimmed.parse::<usize>() {
        if n == 0 {
            return Err(ParseError::Unknown("line numbers are 1-indexed".into()));
        }
        return Ok(ExCmd::GotoLine(n));
    }

    // Argument-bearing commands. Split on the first whitespace so the
    // head is the command and the tail is its (possibly empty) argument.
    let (head, rest) = trimmed
        .split_once(char::is_whitespace)
        .unwrap_or((trimmed, ""));
    let args = rest.trim();
    let force = head.ends_with('!');
    let head_no_bang = head.trim_end_matches('!');

    if matches!(head_no_bang, "e" | "edit") {
        if args.is_empty() {
            // `:e` / `:edit` with no arg — reloading the current buffer
            // is a vim convenience we don't support yet, so flag it as
            // a missing argument.
            return Err(ParseError::MissingArg(
                if force { "e!" } else { "e" }.into(),
            ));
        }
        return Ok(ExCmd::Edit {
            path: args.to_string(),
            force,
        });
    }

    match trimmed {
        "w" => Ok(ExCmd::Write),
        "q" => Ok(ExCmd::Quit { force: false }),
        "q!" => Ok(ExCmd::Quit { force: true }),
        "wq" | "x" => Ok(ExCmd::WriteQuit),
        "noh" | "nohl" | "nohls" | "nohlsearch" => Ok(ExCmd::NoHighlight),
        other => Err(ParseError::Unknown(other.to_string())),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    Unknown(String),
    /// Command recognized but its required argument is missing
    /// (`:e` with no path, etc.).
    MissingArg(String),
}

/// Parse the body of `:%s/<rest>` (the `%s/` prefix has been
/// stripped). Splits on the next two unescaped `/` to extract
/// pattern and replacement; an optional trailing flag segment is
/// accepted but ignored (V1 always operates document-globally).
///
/// Errors land as `ParseError::Unknown` with a hint embedded in
/// the message — they show up in the status footer the same way
/// any other parse error does.
fn parse_substitute(body: &str) -> Result<ExCmd, ParseError> {
    // Find the first `/` not preceded by an escape backslash.
    let mid = next_unescaped_slash(body, 0)
        .ok_or_else(|| ParseError::Unknown("substitute: missing /replacement".into()))?;
    let pattern_part = &body[..mid];
    let after_mid = &body[mid + 1..];
    // The third `/` is optional — `:%s/foo/bar` and `:%s/foo/bar/`
    // are both legal. When present, anything after it is a flag tail
    // (V1 ignores; the command is always doc-global already).
    let end = next_unescaped_slash(after_mid, 0).unwrap_or(after_mid.len());
    let replacement_part = &after_mid[..end];

    if pattern_part.is_empty() {
        return Err(ParseError::Unknown("substitute: empty pattern".into()));
    }
    Ok(ExCmd::Substitute {
        pattern: unescape_slashes(pattern_part),
        replacement: unescape_slashes(replacement_part),
    })
}

/// Find the next `/` byte in `s` starting at `from`, ignoring any
/// `\/` escape pair. Returns the byte index of the matching slash
/// or `None` when no unescaped slash exists.
fn next_unescaped_slash(s: &str, from: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] == b'/' {
            // Walk back through any preceding backslashes to count
            // them — odd count → escape, even → literal.
            let mut backslashes = 0usize;
            let mut j = i;
            while j > 0 && bytes[j - 1] == b'\\' {
                backslashes += 1;
                j -= 1;
            }
            if backslashes % 2 == 0 {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// Replace `\/` with `/` in a delimiter-stripped pattern or
/// replacement segment. Keeps the V1 semantics simple: only `/`
/// is escapable (no `\\` collapse, no regex meta-chars).
fn unescape_slashes(s: &str) -> String {
    s.replace("\\/", "/")
}

/// Run the parsed [`ExCmd`] against `app`. Mutates `app.should_quit`
/// when appropriate. Returns a status message to display.
pub fn execute(app: &mut App, cmd: ExCmd) -> ExResult {
    match cmd {
        ExCmd::Write => {
            // BLOCKS view persists via the per-pane draft (sub-doc →
            // draft → disk), not by serializing the active doc — the
            // sub-doc is only the field text, not a full file. Route
            // through `BlocksSaveDraft` so `:w` lands on the same
            // path Ctrl+S uses.
            if matches!(app.view, crate::app::AppView::Blocks) {
                crate::input::dispatch::apply_action(
                    app,
                    crate::input::action::Action::BlocksSaveDraft,
                    false,
                );
                return ExResult::Ok(String::new());
            }
            match write_document(app) {
                Ok(msg) => ExResult::Ok(msg),
                Err(msg) => ExResult::Err(msg),
            }
        }
        ExCmd::Quit { force } => quit_or_close(app, force),
        ExCmd::WriteQuit => match write_document(app) {
            Ok(_) => quit_or_close(app, /* force = */ true),
            Err(msg) => ExResult::Err(msg),
        },
        ExCmd::Edit { path, force } => match app.open_document(PathBuf::from(path), force) {
            Ok(msg) => ExResult::Ok(msg),
            Err(msg) => ExResult::Err(msg),
        },
        ExCmd::NoHighlight => {
            // Hide matches without losing the pattern — `n`/`N` keep
            // navigating; the next `/`-search re-arms `search_highlight`.
            app.vim.search_highlight = false;
            ExResult::Ok(String::new())
        }
        ExCmd::Substitute {
            pattern,
            replacement,
        } => apply_substitute(app, pattern, replacement),
        ExCmd::GotoLine(n) => {
            // Reuse the motion engine so behavior matches `<n>G`
            // exactly: viewport scrolls, cursor lands on the first
            // non-blank of the target line, etc.
            let viewport = app.viewport_height();
            if let Some(doc) = app.document_mut() {
                crate::vim::motions::apply(
                    crate::vim::parser::Motion::GotoLine(n),
                    doc,
                    1,
                    viewport,
                );
            }
            app.refresh_viewport_for_cursor();
            ExResult::Ok(String::new())
        }
    }
}

/// Execute `:%s/pattern/replacement`. Strategy: serialize the doc
/// to markdown, run a literal `replace`, count occurrences, build a
/// fresh `Document::from_markdown`. The cursor lands on doc-start
/// post-substitution — keeping it stable across a wholesale
/// re-parse would mean tracking positions through arbitrary edits,
/// which is out of scope for V1 (the user typed the chord, they
/// know they're moving).
fn apply_substitute(app: &mut App, pattern: String, replacement: String) -> ExResult {
    if pattern.is_empty() {
        return ExResult::Err("substitute: empty pattern".into());
    }
    let Some(doc) = app.tabs.active_document_mut() else {
        return ExResult::Err("no buffer".into());
    };
    let before = doc.to_markdown();
    let count = before.matches(&pattern).count();
    if count == 0 {
        return ExResult::Ok(format!("pattern not found: {pattern}"));
    }
    let after = before.replace(&pattern, &replacement);
    // `Document::from_markdown` re-parses block fences, so renaming
    // an alias used in `{{alias.x}}` refs is reflected by the
    // dependency-resolution layer on the next run.
    match crate::buffer::Document::from_markdown(&after) {
        Ok(new_doc) => {
            *doc = new_doc;
            // Mark dirty explicitly — `from_markdown` builds a clean
            // doc, but the in-memory state now diverges from disk
            // until the user `:w`s.
            doc.mark_dirty();
            ExResult::Ok(format!("{count} substitutions"))
        }
        Err(e) => ExResult::Err(format!("substitute reparse failed: {e}")),
    }
}

/// Convenience: parse + execute in one call. The cmdline buffer
/// passed in must NOT include the leading `:`.
pub fn run(app: &mut App, buf: &str) -> ExResult {
    match parse(buf) {
        Ok(cmd) => execute(app, cmd),
        Err(ParseError::Empty) => ExResult::Empty,
        Err(ParseError::Unknown(s)) => ExResult::Unknown(s),
        Err(ParseError::MissingArg(cmd)) => {
            ExResult::Err(format!("E471: Argument required for :{cmd}"))
        }
    }
}

/// Behavior of `:q` / `:wq` / `:x`:
///
/// 1. The active tab has more than one pane → close the focused split
///    (vim native window-close).
/// 2. Otherwise → close the active tab.
/// 3. The last tab just closed → quit the app.
///
/// `force == false` rejects when the closed unit has dirty content.
fn quit_or_close(app: &mut App, force: bool) -> ExResult {
    let leaf_count = app.active_tab().map(|t| t.leaf_count()).unwrap_or(0);
    if leaf_count > 1 {
        // Closing a split. Refuse only when the *focused* pane is dirty
        // — sibling splits are unaffected.
        if !force && app.document().is_some_and(|d| d.is_dirty()) {
            return ExResult::Err("no write since last change (add ! to override)".into());
        }
        if let Some(tab) = app.active_tab_mut() {
            tab.close_focused();
        }
        return ExResult::Ok(String::new());
    }
    // Single pane in the tab: close the whole tab. close_tab() handles
    // its own dirty check.
    match app.close_tab(force) {
        Ok(msg) => {
            if app.tabs.is_empty() {
                app.should_quit = true;
                ExResult::Quit
            } else {
                ExResult::Ok(msg)
            }
        }
        Err(msg) => ExResult::Err(msg),
    }
}

fn write_document(app: &mut App) -> Result<String, String> {
    let Some(file) = app.document_path().cloned() else {
        return Err("no file name".into());
    };
    let Some(doc) = app.tabs.active_document_mut() else {
        return Err("no buffer".into());
    };
    let body = doc.to_markdown();
    // `document_path` is stored relative to the active vault (matches
    // how `pick_initial_file` and `read_note` work). Reuse `write_note`
    // so the vault join + parent-dir creation logic stays in one place.
    let vault = app.vault_path.to_string_lossy().into_owned();
    let file_str = file.to_string_lossy().into_owned();
    httui_core::fs::write_note(&vault, &file_str, &body)
        .map_err(|e| format!("write failed: {e}"))?;
    doc.mark_clean();
    let bytes = body.len();
    let lines = body.lines().count();

    // Keep the FTS5 search index fresh — only after the user has
    // opened the search modal at least once (otherwise we'd be
    // writing rows the user never queries). When `<C-f>` first
    // opens, `rebuild_search_index` does a full sweep that picks
    // up any saves we skipped in this branch. `.md` only — other
    // file types aren't indexed.
    if app.content_search_index_built && file_str.ends_with(".md") {
        let pool = app.pool_manager.app_pool().clone();
        let path_for_index = file_str.clone();
        let body_for_index = body.clone();
        tokio::spawn(async move {
            if let Err(e) =
                httui_core::search::update_search_entry(&pool, &path_for_index, &body_for_index)
                    .await
            {
                tracing::warn!("search index update failed: {e}");
            }
        });
    }

    let name = file
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or(file_str);
    Ok(format!("\"{name}\" {lines}L, {bytes}B written"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_known_commands() {
        assert_eq!(parse("w"), Ok(ExCmd::Write));
        assert_eq!(parse("q"), Ok(ExCmd::Quit { force: false }));
        assert_eq!(parse("q!"), Ok(ExCmd::Quit { force: true }));
        assert_eq!(parse("wq"), Ok(ExCmd::WriteQuit));
        assert_eq!(parse("x"), Ok(ExCmd::WriteQuit));
    }

    #[test]
    fn parse_noh_aliases() {
        for alias in ["noh", "nohl", "nohls", "nohlsearch"] {
            assert_eq!(parse(alias), Ok(ExCmd::NoHighlight));
        }
    }

    #[test]
    fn parse_trims_whitespace() {
        assert_eq!(parse("  w  "), Ok(ExCmd::Write));
    }

    #[test]
    fn parse_empty_is_error() {
        assert_eq!(parse(""), Err(ParseError::Empty));
        assert_eq!(parse("   "), Err(ParseError::Empty));
    }

    #[test]
    fn parse_unknown() {
        match parse("frobnicate") {
            Err(ParseError::Unknown(s)) => assert_eq!(s, "frobnicate"),
            other => panic!("expected unknown, got {other:?}"),
        }
    }

    #[test]
    fn parse_explain_no_longer_recognized() {
        // EXPLAIN moved off ex commands and onto the `<C-x>` keymap
        // (per project directive: surface new actions as keymaps,
        // not ex commands). `:explain` / `:exp` must report unknown
        // so users notice the rebind instead of silently dropping
        // the keystroke into a no-op.
        for alias in ["explain", "exp"] {
            match parse(alias) {
                Err(ParseError::Unknown(s)) => assert_eq!(s, alias),
                other => panic!("expected unknown for `:{alias}`, got {other:?}"),
            }
        }
    }

    #[test]
    fn parse_edit_with_path() {
        assert_eq!(
            parse("e foo.md"),
            Ok(ExCmd::Edit {
                path: "foo.md".into(),
                force: false
            })
        );
        assert_eq!(
            parse("edit notes/today.md"),
            Ok(ExCmd::Edit {
                path: "notes/today.md".into(),
                force: false
            })
        );
    }

    #[test]
    fn parse_edit_force_variants() {
        assert_eq!(
            parse("e! foo.md"),
            Ok(ExCmd::Edit {
                path: "foo.md".into(),
                force: true
            })
        );
        assert_eq!(
            parse("edit! foo.md"),
            Ok(ExCmd::Edit {
                path: "foo.md".into(),
                force: true
            })
        );
    }

    #[test]
    fn parse_edit_missing_arg() {
        match parse("e") {
            Err(ParseError::MissingArg(s)) => assert_eq!(s, "e"),
            other => panic!("expected missing arg, got {other:?}"),
        }
        match parse("e!") {
            Err(ParseError::MissingArg(s)) => assert_eq!(s, "e!"),
            other => panic!("expected missing arg, got {other:?}"),
        }
        match parse("edit") {
            Err(ParseError::MissingArg(s)) => assert_eq!(s, "e"),
            other => panic!("expected missing arg, got {other:?}"),
        }
    }

    #[test]
    fn parse_no_longer_recognizes_file_op_commands() {
        // `:new`, `:mv`, `:rm` are features now, not vim natives.
        // The cmdline parser must reject them so users learn to use the
        // tree shortcuts (`a`/`r`/`d`).
        assert!(matches!(parse("new foo.md"), Err(ParseError::Unknown(_))));
        assert!(matches!(parse("mv foo.md"), Err(ParseError::Unknown(_))));
        assert!(matches!(parse("rm! foo.md"), Err(ParseError::Unknown(_))));
    }

    #[test]
    fn parse_no_longer_recognizes_tab_commands() {
        // Tab management is a TUI feature now — driven by Ctrl+T (new
        // tab via Quick Open) and Ctrl+W (close tab). The cmdline
        // parser must reject the old aliases so they stay one source
        // of truth.
        assert!(matches!(
            parse("tabnew foo.md"),
            Err(ParseError::Unknown(_))
        ));
        assert!(matches!(parse("tabclose"), Err(ParseError::Unknown(_))));
    }

    #[test]
    fn parse_substitute_basic_form() {
        assert_eq!(
            parse("%s/foo/bar"),
            Ok(ExCmd::Substitute {
                pattern: "foo".into(),
                replacement: "bar".into(),
            })
        );
    }

    #[test]
    fn parse_substitute_trailing_slash_and_flags_ignored() {
        // `:%s/foo/bar/` and `:%s/foo/bar/g` both legal; flag tail
        // ignored since V1 is always doc-global.
        assert_eq!(
            parse("%s/foo/bar/"),
            Ok(ExCmd::Substitute {
                pattern: "foo".into(),
                replacement: "bar".into(),
            })
        );
        assert_eq!(
            parse("%s/foo/bar/g"),
            Ok(ExCmd::Substitute {
                pattern: "foo".into(),
                replacement: "bar".into(),
            })
        );
    }

    #[test]
    fn parse_substitute_unescapes_slash_in_pattern_and_replacement() {
        // `\/` in either segment becomes a literal `/`, so users can
        // rename `path/old` → `path/new`.
        assert_eq!(
            parse("%s/path\\/old/path\\/new"),
            Ok(ExCmd::Substitute {
                pattern: "path/old".into(),
                replacement: "path/new".into(),
            })
        );
    }

    #[test]
    fn parse_substitute_empty_pattern_errors() {
        match parse("%s//bar") {
            Err(ParseError::Unknown(msg)) => assert!(msg.contains("empty pattern")),
            other => panic!("expected empty-pattern error, got {other:?}"),
        }
    }

    #[test]
    fn parse_substitute_missing_replacement_segment_errors() {
        match parse("%s/foo") {
            Err(ParseError::Unknown(msg)) => assert!(msg.contains("missing /replacement")),
            other => panic!("expected missing-replacement error, got {other:?}"),
        }
    }

    #[test]
    fn parse_bare_number_is_goto_line() {
        assert_eq!(parse("42"), Ok(ExCmd::GotoLine(42)));
        assert_eq!(parse("1"), Ok(ExCmd::GotoLine(1)));
        // Whitespace tolerated.
        assert_eq!(parse("  100  "), Ok(ExCmd::GotoLine(100)));
    }

    #[test]
    fn parse_zero_line_errors() {
        // Vim line numbers are 1-indexed; reject `:0` so the user
        // doesn't get silent off-by-one behavior.
        match parse("0") {
            Err(ParseError::Unknown(msg)) => assert!(msg.contains("1-indexed")),
            other => panic!("expected 1-indexed hint, got {other:?}"),
        }
    }

    async fn app_with_doc(md: &str) -> (App, tempfile::TempDir, tempfile::TempDir) {
        let data = tempfile::TempDir::new().unwrap();
        let vault = tempfile::TempDir::new().unwrap();
        std::fs::write(vault.path().join("a.md"), md).unwrap();
        let pool = httui_core::db::init_db(data.path()).await.unwrap();
        let mut app = App::new(
            crate::config::Config::default(),
            crate::vault::ResolvedVault {
                vault: vault.path().to_path_buf(),
            },
            pool,
        );
        let doc = crate::buffer::Document::from_markdown(md).unwrap();
        let pane = crate::pane::Pane::new(doc, vault.path().join("a.md"));
        app.tabs.tabs = vec![crate::pane::TabState::new(pane)];
        app.tabs.active = 0;
        (app, data, vault)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn run_noh_clears_highlight() {
        let (mut app, _d, _v) = app_with_doc("hello\n").await;
        app.vim.search_highlight = true;
        assert!(matches!(run(&mut app, "noh"), ExResult::Ok(_)));
        assert!(!app.vim.search_highlight);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn run_goto_line_is_accepted() {
        let (mut app, _d, _v) = app_with_doc("l1\nl2\nl3\n").await;
        assert!(matches!(
            run(&mut app, "2"),
            ExResult::Ok(_) | ExResult::Empty
        ));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn run_substitute_replaces_in_document() {
        let (mut app, _d, _v) = app_with_doc("foo foo bar\n").await;
        assert!(matches!(
            run(&mut app, "%s/foo/baz/"),
            ExResult::Ok(_) | ExResult::Err(_)
        ));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn run_write_and_wq_dispatch() {
        let (mut app, _d, _v) = app_with_doc("save me\n").await;
        assert!(matches!(
            run(&mut app, "w"),
            ExResult::Ok(_) | ExResult::Err(_)
        ));
        let _ = run(&mut app, "wq");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn run_quit_closes_tab() {
        let (mut app, _d, _v) = app_with_doc("bye\n").await;
        assert!(matches!(
            run(&mut app, "q!"),
            ExResult::Quit | ExResult::Ok(_) | ExResult::Err(_)
        ));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn run_unknown_and_empty() {
        let (mut app, _d, _v) = app_with_doc("x\n").await;
        assert!(matches!(run(&mut app, "frobnicate"), ExResult::Unknown(_)));
        assert!(matches!(run(&mut app, ""), ExResult::Empty));
    }
}
