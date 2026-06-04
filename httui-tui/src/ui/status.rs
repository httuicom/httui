use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::{App, StatusKind};
use crate::buffer::{Cursor, Document, Segment};
use crate::tree::TreePromptKind;
use crate::vim::mode::Mode;

pub fn render_status_bar(frame: &mut Frame, area: Rect, app: &App) {
    // Priority: command-line prompt > tree prompt > search prompt > status > info.
    if app.vim.mode == Mode::CommandLine {
        let buf = app
            .modal
            .as_ref()
            .and_then(|m| m.as_prompt())
            .map(|(_, le)| le.as_str().to_string())
            .unwrap_or_default();
        let line = Line::from(vec![Span::raw(format!(":{buf}"))]);
        frame.render_widget(Paragraph::new(line), area);
        return;
    }

    if app.vim.mode == Mode::TreePrompt {
        if let Some(prompt) = app.tree.prompt.as_ref() {
            let label = match &prompt.kind {
                TreePromptKind::Create { dir } => {
                    if dir.is_empty() {
                        "new file: ".to_string()
                    } else {
                        format!("new file in {dir}/: ")
                    }
                }
                TreePromptKind::Rename { from } => format!("rename {from} → "),
                TreePromptKind::Delete { target } => {
                    format!("delete {target}? (y/N) ")
                }
                TreePromptKind::DeleteBlock { label, .. } => {
                    format!("delete block {label}? (y/N) ")
                }
            };
            let line = Line::from(vec![
                Span::styled(
                    label,
                    Style::default().fg(crate::ui::palette::popup_border_accent()),
                ),
                Span::raw(prompt.buffer().to_string()),
            ]);
            frame.render_widget(Paragraph::new(line), area);
        }
        return;
    }

    if app.vim.mode == Mode::Search {
        let (sigil, buf) = match app.modal.as_ref().and_then(|m| m.as_prompt()) {
            Some((crate::modal::PromptKind::Search { forward }, le)) => {
                (if forward { '/' } else { '?' }, le.as_str().to_string())
            }
            _ => ('/', String::new()),
        };
        let line = Line::from(vec![Span::raw(format!("{sigil}{buf}"))]);
        frame.render_widget(Paragraph::new(line), area);
        return;
    }

    // Mode::FenceEdit renders as a popup over the block (see
    // `ui::fence_edit`), not in the status bar. We deliberately don't
    // handle that mode here — falling through paints the normal
    // file/vault status line so the user keeps a visible reference
    // to which file the popup is editing.

    if let Some(msg) = app.status_message.as_ref() {
        let style = match msg.kind {
            StatusKind::Info => Style::default(),
            StatusKind::Error => Style::default()
                .fg(crate::ui::palette::foreground())
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
        };
        let line = Line::from(vec![Span::styled(format!(" {}", msg.text), style)]);
        frame.render_widget(Paragraph::new(line), area);
        return;
    }

    let vault = compact_vault_path(&app.vault_path);
    let dirty_marker = if app.document().is_some_and(|d| d.is_dirty()) {
        " ·●"
    } else {
        ""
    };
    let file = app
        .document_path()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "(no file)".into());

    let block_count = app.document().map(count_blocks).unwrap_or(0);

    let cursor_label = app
        .document()
        .map(describe_cursor)
        .unwrap_or_else(|| "—".into());

    let mode = app.vim.mode;
    // Running indicator — emits while a DB/HTTP execution is in
    // flight. Painted right after the mode chip so the user has
    // visible feedback even after navigating away from the source
    // block (Ctrl-W h, gt, etc.). Green to suggest "alive"; the
    // elapsed counter ticks once per Tick (250 ms) which is fine
    // for human "is something happening" feedback.
    let running_chip: Vec<Span<'static>> = match running_chip_label(app) {
        Some(label) => vec![
            Span::raw(" "),
            Span::styled(
                format!(" {label} "),
                Style::default()
                    .fg(crate::ui::palette::popup_bg())
                    .bg(crate::ui::palette::success())
                    .add_modifier(Modifier::BOLD),
            ),
        ],
        None => Vec::new(),
    };
    // Active environment chip — only emits when an env is set as
    // active; otherwise we skip the section entirely so the status
    // bar stays compact for vaults that don't use envs.
    let env_chip: Vec<Span<'static>> = match app.active_env_name.as_deref() {
        Some(name) => vec![
            Span::raw(" "),
            Span::styled(
                format!(" env: {name} "),
                Style::default()
                    .fg(crate::ui::palette::popup_bg())
                    .bg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD),
            ),
        ],
        None => Vec::new(),
    };
    // Git branch chip — branch + ahead/behind. Hidden in non-git vaults.
    let git_chip: Vec<Span<'static>> = match git_chip_label(app) {
        Some(label) => vec![
            Span::raw(" "),
            Span::styled(
                format!(" {label} "),
                Style::default()
                    .fg(crate::ui::palette::popup_bg())
                    .bg(Color::LightBlue)
                    .add_modifier(Modifier::BOLD),
            ),
        ],
        None => Vec::new(),
    };
    // Focused-block connection chip — only emits when the cursor is
    // parked on a DB block with a resolvable connection_id. Mirrors
    // the env chip: cyan instead of magenta to keep the two visually
    // distinct. We deliberately don't show "conn: <id>" for unresolved
    // ids — the missing-connection error surfaces at run time, and a
    // chip here would just add noise.
    let conn_chip: Vec<Span<'static>> = match focused_block_conn_name(app) {
        Some(name) => vec![
            Span::raw(" "),
            Span::styled(
                format!(" conn: {name} "),
                Style::default()
                    .fg(crate::ui::palette::popup_bg())
                    .bg(Color::LightCyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ],
        None => Vec::new(),
    };
    // Mode chip is a vim affordance: it only means something when the
    // modal engine is driving input. In Standard editor mode there are
    // no vim modes to surface, so we skip the chip entirely and let the
    // status bar lead with the file/vault section.
    let mut spans: Vec<Span<'static>> = if app.config.editor.mode == crate::config::EditorMode::Vim
    {
        vec![Span::styled(
            format!(" {} ", mode.label()),
            Style::default()
                .fg(crate::ui::palette::popup_bg())
                .bg(mode.bg())
                .add_modifier(Modifier::BOLD),
        )]
    } else {
        Vec::new()
    };
    spans.extend(blocks_view_chips(app));
    // In BLOCKS view the running indicator lives inside the pane (on
    // the focused block's `[4] Response` / `[3] Result` title) so the
    // bytes/elapsed sit next to where the response will land. Skip it
    // in the global footer to avoid duplication.
    if !matches!(app.view, crate::app::AppView::Blocks) {
        spans.extend(running_chip);
    }
    spans.extend(git_chip);
    spans.extend(env_chip);
    // The connection is a per-block property (shown in each block's
    // header). A single global chip is misleading when panes hold
    // blocks on different connections, so skip it in BLOCKS view.
    if !matches!(app.view, crate::app::AppView::Blocks) {
        spans.extend(conn_chip);
    }
    // pending-secrets badge. Only emits when the active
    // vault has refs without a keychain entry; clicking is replaced
    // by the `s` chord inside the vault picker (Alt+; → s reopens
    // the first-run modal so the user can fill them in).
    if !app.pending_secrets.is_empty() {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!(" ! {} secrets ", app.pending_secrets.len()),
            Style::default()
                .fg(crate::ui::palette::popup_bg())
                .bg(crate::ui::palette::popup_border_accent())
                .add_modifier(Modifier::BOLD),
        ));
    }
    if matches!(app.view, crate::app::AppView::Blocks) {
        // Push the version tail flush-right so the mode/context group on
        // the left gets room to breathe instead of all crowding the edge.
        let right = blocks_view_status_tail();
        let left_w: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        let right_w: usize = right.iter().map(|s| s.content.chars().count()).sum();
        let total = area.width as usize;
        if total > left_w + right_w {
            spans.push(Span::raw(" ".repeat(total - left_w - right_w)));
        }
        spans.extend(right);
    } else {
        spans.push(Span::raw(format!(
            " {file}{dirty_marker} · {block_count} blocks · {cursor_label} · vault: {vault} · theme: {}",
            app.config.theme
        )));
    }
    let line = Line::from(spans);
    frame.render_widget(Paragraph::new(line), area);
}

/// Trailing status segment for BLOCKS view — just the binary version.
/// Open-block count and split shape were dropped: the split is obvious
/// from the layout and the count added noise.
fn blocks_view_status_tail() -> Vec<Span<'static>> {
    let muted = Style::default().fg(crate::ui::palette::muted());
    let version = env!("CARGO_PKG_VERSION");
    vec![Span::styled(format!(" httui {version}"), muted)]
}

/// Render a compact form of the vault path for the status bar.
/// Two-step compaction:
///
/// 1. Replace a `$HOME` prefix with `~` so a vault under the user's
///    home directory loses 30+ characters.
/// 2. If the result is still longer than 40 chars, truncate to the
///    last two path components prefixed with `…/` so `…/projects/notes`.
///
/// Both rules are pure-string — they don't probe the filesystem, so
/// they're safe to call every render.
fn compact_vault_path(path: &std::path::Path) -> String {
    let raw = path.to_string_lossy().into_owned();
    let with_home = match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && raw.starts_with(&home) => {
            format!("~{}", &raw[home.len()..])
        }
        _ => raw,
    };
    if with_home.chars().count() <= 40 {
        return with_home;
    }
    // Walk from the end, taking the last two non-empty segments.
    let segments: Vec<&str> = with_home.split('/').filter(|s| !s.is_empty()).collect();
    if segments.len() < 2 {
        return with_home;
    }
    let tail = segments[segments.len() - 2..].join("/");
    format!("…/{tail}")
}

/// BLOCKS-view-only chips: `NAV · Region` while navigating, or
/// `EDIT · field` once a buffer is open. The view is already obvious
/// from the split layout, so no `BLOCKS` chip. Empty in DOC view.
fn blocks_view_chips(app: &App) -> Vec<Span<'static>> {
    if !matches!(app.view, crate::app::AppView::Blocks) {
        return Vec::new();
    }
    let fg = crate::ui::palette::popup_bg();
    let mut out: Vec<Span<'static>> = Vec::new();
    let Some(pane) = app.active_pane() else {
        return out;
    };
    let block_type = pane.block_selected.and_then(|sel| {
        app.blocks_workspace.as_ref().and_then(|ws| {
            ws.index
                .files
                .get(sel.file_idx)
                .and_then(|f| f.blocks.get(sel.block_idx))
                .map(|b| b.block_type.clone())
        })
    });
    let region_idx = pane.block_region;
    let region_name = block_type
        .as_deref()
        .map(|bt| crate::app::region_label(bt, region_idx))
        .unwrap_or("");
    if let Some(edit) = pane.block_edit.as_ref() {
        let field = field_label(&edit.field);
        let chip = Style::default()
            .bg(crate::ui::palette::amber())
            .fg(fg)
            .add_modifier(Modifier::BOLD);
        let chip_label = match (
            app.config.editor.mode,
            crate::input::apply::blocks_view::effective_sub_mode(app),
        ) {
            (crate::config::EditorMode::Vim, crate::app::EditSubMode::Normal) => " vim NORMAL ",
            (crate::config::EditorMode::Vim, crate::app::EditSubMode::Insert) => " vim INSERT ",
            _ => " EDIT ",
        };
        out.push(Span::raw(" "));
        out.push(Span::styled(chip_label, chip));
        out.push(Span::raw(" "));
        out.push(Span::styled(
            format!("{region_name} · {field}"),
            Style::default().fg(crate::ui::palette::muted()),
        ));
    } else {
        out.push(Span::raw(" "));
        out.push(Span::styled(
            "NAV",
            Style::default().fg(crate::ui::palette::accent()),
        ));
        out.push(Span::raw(" "));
        out.push(Span::styled(
            region_name.to_string(),
            Style::default().fg(crate::ui::palette::muted()),
        ));
    }
    out
}

fn field_label(field: &crate::app::EditField) -> &'static str {
    match field {
        crate::app::EditField::HttpUrl => "url",
        crate::app::EditField::HttpHeaderKey(_) => "header key",
        crate::app::EditField::HttpHeaderValue(_) => "header value",
        crate::app::EditField::HttpBody => "body",
        crate::app::EditField::DbQuery => "query",
    }
}

/// Build the running-indicator chip label, or `None` when no
/// query / HTTP request is in flight. Format: `▶ DB · 2.3s` or
/// `▶ HTTP · 1.1s`. Block-type comes from the source block at
/// `RunningQuery.segment_idx`; if the document has shifted under
/// the in-flight task and the segment isn't a block anymore, we
/// fall back to a generic `▶ running · Xs` so the user still sees
/// "something is happening".
pub(crate) fn running_chip_label(app: &App) -> Option<String> {
    let rq = app.running_query.as_ref()?;
    let elapsed = rq.started_at.elapsed().as_secs_f32();
    let kind = app
        .document()
        .and_then(|d| d.segments().get(rq.segment_idx))
        .and_then(|s| match s {
            Segment::Block(b) if b.is_http() => Some("HTTP"),
            Segment::Block(b) if b.is_db() => Some("DB"),
            _ => None,
        })
        .unwrap_or("running");
    if kind == "HTTP" && rq.bytes_received > 0 {
        Some(format!(
            "▶ HTTP · ↓ {} · {elapsed:.1}s",
            format_progress_bytes(rq.bytes_received)
        ))
    } else {
        Some(format!("▶ {kind} · {elapsed:.1}s"))
    }
}

/// Git chip label: `"<branch>"` in sync, `"<branch> ↑N ↓M"` when
/// diverged. `None` when no snapshot yet / not a git repo.
fn git_chip_label(app: &App) -> Option<String> {
    let status = app.git_panel.status.as_ref()?;
    let branch = status.branch.as_deref().unwrap_or("detached");
    if status.ahead == 0 && status.behind == 0 {
        Some(branch.to_string())
    } else {
        Some(format!("{branch} ↑{} ↓{}", status.ahead, status.behind))
    }
}

fn format_progress_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} kB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn count_blocks(doc: &Document) -> usize {
    doc.segments()
        .iter()
        .filter(|s| matches!(s, Segment::Block(_)))
        .count()
}

/// Resolve the human label of the connection used by the DB block
/// the cursor is currently parked on. Returns `None` when:
/// - there's no document open
/// - the cursor isn't on a block (or the block isn't a DB block)
/// - the block has no `connection` / `connection_id` param
/// - the id doesn't resolve in the global `connection_names` map
///   (deleted / stale / typo'd id — surfaces as a missing chip
///   rather than `conn: <uuid>` so the user notices the breakage).
fn focused_block_conn_name(app: &App) -> Option<String> {
    let doc = app.document()?;
    let segment_idx = match doc.cursor() {
        Cursor::InBlock { segment_idx, .. } | Cursor::InBlockResult { segment_idx, .. } => {
            segment_idx
        }
        _ => return None,
    };
    let block = match doc.segments().get(segment_idx)? {
        Segment::Block(b) => b,
        _ => return None,
    };
    if !block.is_db() {
        return None;
    }
    // Match the lookup priority used by the dispatcher when picking
    // a connection (see `apply_confirm_connection_picker`): newer
    // blocks store `connection`; pre-redesign ones used
    // `connection_id`. Either resolves through the same name map.
    let id = block
        .params
        .get("connection")
        .or_else(|| block.params.get("connection_id"))
        .and_then(|v| v.as_str())?;
    app.connection_names.get(id).cloned()
}

fn describe_cursor(doc: &Document) -> String {
    match doc.cursor() {
        Cursor::InProse {
            segment_idx,
            offset,
        } => {
            if let Some(Segment::Prose(rope)) = doc.segments().get(segment_idx) {
                let off = offset.min(rope.len_chars());
                let line = rope.char_to_line(off) + 1;
                let col = off - rope.line_to_char(line - 1) + 1;
                format!("Ln {line} Col {col}")
            } else {
                "Ln ? Col ?".into()
            }
        }
        Cursor::InBlock {
            segment_idx,
            offset,
        } => {
            use crate::buffer::block::{raw_section_at, RawSection};
            let block_idx = doc
                .segments()
                .iter()
                .take(segment_idx + 1)
                .filter(|s| matches!(s, Segment::Block(_)))
                .count();
            let raw = match doc.segments().get(segment_idx) {
                Some(Segment::Block(b)) => &b.raw,
                _ => return format!("Block #{block_idx} · ?"),
            };
            match raw_section_at(raw, offset) {
                RawSection::Header => format!("Block #{block_idx} · fence ```"),
                RawSection::Closer => format!("Block #{block_idx} · fence ```"),
                RawSection::Body { line, col } => {
                    format!("Block #{block_idx} · Ln {} Col {}", line + 1, col + 1)
                }
            }
        }
        Cursor::InBlockResult { segment_idx, row } => {
            let block_idx = doc
                .segments()
                .iter()
                .take(segment_idx + 1)
                .filter(|s| matches!(s, Segment::Block(_)))
                .count();
            format!("Block #{block_idx} · Result row {}", row + 1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, RunningKind, RunningQuery, StatusMessage};
    use crate::buffer::Cursor;
    use crate::config::{Config, EditorMode};
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::style::{Color, Modifier};
    use ratatui::Terminal;
    use std::path::PathBuf;
    use std::time::Instant;
    use tempfile::TempDir;
    use tokio_util::sync::CancellationToken;

    // ---- compact_vault_path (pure-string, no App needed) -------------

    #[test]
    fn short_paths_stay_intact() {
        assert_eq!(compact_vault_path(&PathBuf::from("/notes")), "/notes");
    }

    #[test]
    fn long_paths_collapse_to_tail() {
        // Pure-string path: > 40 chars and no HOME prefix → tail
        // truncation should kick in.
        let p = PathBuf::from("/very/deeply/nested/repository/projects/work/notes");
        let out = compact_vault_path(&p);
        assert_eq!(out, "…/work/notes");
    }

    #[test]
    fn tail_truncation_handles_double_slashes() {
        // Empty segments from `//` are filtered out by the walker.
        let p = PathBuf::from("/a/b//c/d/e/f/g/h/i/j/k/l/m/n/o/p/q/r/s/t/u/v/w/x/y/z");
        let out = compact_vault_path(&p);
        // Tail picks the two last non-empty segments.
        assert_eq!(out, "…/y/z");
    }

    #[test]
    fn home_prefix_collapses_to_tilde() {
        // SAFETY: tests in this module are not parallel-sensitive on
        // $HOME — each sets it right before asserting and reads it
        // synchronously inside the same call. `compact_vault_path`
        // reads `$HOME` once via `std::env::var`.
        let prev = std::env::var("HOME").ok();
        unsafe { std::env::set_var("HOME", "/Users/tester") };
        let out = compact_vault_path(&PathBuf::from("/Users/tester/notes"));
        assert_eq!(out, "~/notes");
        match prev {
            Some(v) => unsafe { std::env::set_var("HOME", v) },
            None => unsafe { std::env::remove_var("HOME") },
        }
    }

    // ---- App fixture + render harness --------------------------------

    /// Build an `App` over a vault seeded with the given
    /// `(relative_path, contents)` files. Mirrors the fixture in
    /// `app/impl_file_tab.rs`. `App::new` uses `block_in_place` →
    /// multi-thread runtime required.
    async fn app_with_files(files: &[(&str, &str)]) -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        for (rel, body) in files {
            let p = vault.path().join(rel);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(p, body).unwrap();
        }
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let app = App::new(Config::default(), resolved, pool);
        (app, data, vault)
    }

    /// Render the status bar into a 200×1 test buffer and return the
    /// flattened row text plus the buffer for per-cell style asserts.
    fn render(app: &App) -> (String, ratatui::buffer::Buffer) {
        let backend = TestBackend::new(200, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let area = Rect::new(0, 0, 200, 1);
                render_status_bar(f, area, app);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let text: String = (0..200)
            .map(|x| buf.cell((x, 0)).unwrap().symbol().to_string())
            .collect::<String>();
        (text, buf)
    }

    // ---- prompt branches --------------------------------------------

    #[tokio::test(flavor = "multi_thread")]
    async fn command_line_prompt_renders_colon_prefix() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
        app.vim.mode = Mode::CommandLine;
        app.modal = Some(crate::modal::Modal::Prompt(
            crate::modal::PromptKind::Cmdline,
            crate::vim::lineedit::LineEdit::from_str("w foo"),
        ));
        let (text, _) = render(&app);
        assert!(text.contains(":w foo"), "got: {text:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tree_prompt_create_at_root_and_in_dir() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
        app.vim.mode = Mode::TreePrompt;
        app.tree.prompt = Some(crate::tree::TreePrompt::new(
            TreePromptKind::Create { dir: String::new() },
            "draft.md".into(),
        ));
        let (text, _) = render(&app);
        assert!(text.contains("new file: "), "got: {text:?}");
        assert!(text.contains("draft.md"), "got: {text:?}");

        app.tree.prompt = Some(crate::tree::TreePrompt::new(
            TreePromptKind::Create {
                dir: "notes".into(),
            },
            String::new(),
        ));
        let (text, _) = render(&app);
        assert!(text.contains("new file in notes/: "), "got: {text:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tree_prompt_rename_and_delete_labels() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
        app.vim.mode = Mode::TreePrompt;
        app.tree.prompt = Some(crate::tree::TreePrompt::new(
            TreePromptKind::Rename {
                from: "old.md".into(),
            },
            "old.md".into(),
        ));
        let (text, _) = render(&app);
        assert!(text.contains("rename old.md → "), "got: {text:?}");

        app.tree.prompt = Some(crate::tree::TreePrompt::new(
            TreePromptKind::Delete {
                target: "gone.md".into(),
            },
            String::new(),
        ));
        let (text, _) = render(&app);
        assert!(text.contains("delete gone.md? (y/N) "), "got: {text:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tree_prompt_mode_without_prompt_renders_blank() {
        // Mode is TreePrompt but `tree.prompt` is None — the function
        // returns early without painting anything.
        let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
        app.vim.mode = Mode::TreePrompt;
        app.tree.prompt = None;
        let (text, _) = render(&app);
        assert!(text.trim().is_empty(), "expected blank, got: {text:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn search_prompt_forward_and_backward() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
        app.vim.mode = Mode::Search;
        app.modal = Some(crate::modal::Modal::Prompt(
            crate::modal::PromptKind::Search { forward: true },
            crate::vim::lineedit::LineEdit::from_str("needle"),
        ));
        let (text, _) = render(&app);
        assert!(text.contains("/needle"), "got: {text:?}");

        app.modal = Some(crate::modal::Modal::Prompt(
            crate::modal::PromptKind::Search { forward: false },
            crate::vim::lineedit::LineEdit::from_str("needle"),
        ));
        let (text, _) = render(&app);
        assert!(text.contains("?needle"), "got: {text:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn status_message_info_and_error_styles() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
        app.vim.mode = Mode::Normal;
        app.status_message = Some(StatusMessage {
            text: "wrote a.md".into(),
            kind: StatusKind::Info,
        });
        let (text, buf) = render(&app);
        assert!(text.contains("wrote a.md"), "got: {text:?}");
        // Info: default style (no red bg).
        let info_cell = buf.cell((2, 0)).unwrap();
        assert_ne!(info_cell.bg, Color::Red);

        app.status_message = Some(StatusMessage {
            text: "boom".into(),
            kind: StatusKind::Error,
        });
        let (text, buf) = render(&app);
        assert!(text.contains("boom"), "got: {text:?}");
        // Error: white-on-red bold somewhere in the painted text.
        let has_red = (0..20).any(|x| {
            let c = buf.cell((x, 0)).unwrap();
            c.bg == Color::Red && c.modifier.contains(Modifier::BOLD)
        });
        assert!(has_red, "expected a red bold cell in {text:?}");
    }

    // ---- default status line + mode chip guard ----------------------

    #[tokio::test(flavor = "multi_thread")]
    async fn vim_mode_shows_mode_chip_with_label_and_color() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "hello\n")]).await;
        app.config.editor.mode = EditorMode::Vim;
        app.vim.mode = Mode::Normal;
        let (text, buf) = render(&app);
        // Chip carries the NOR label.
        assert!(text.contains("NOR"), "got: {text:?}");
        // First chip cell painted with the mode background + bold.
        let chip = buf.cell((1, 0)).unwrap();
        assert_eq!(chip.bg, Mode::Normal.bg());
        assert!(chip.modifier.contains(Modifier::BOLD));
        // Default-status tail also present.
        assert!(text.contains("a.md"), "got: {text:?}");
        assert!(text.contains("blocks"), "got: {text:?}");
        assert!(text.contains("vault:"), "got: {text:?}");
        assert!(text.contains("theme:"), "got: {text:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn standard_mode_hides_mode_chip() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "hello\n")]).await;
        app.config.editor.mode = EditorMode::Standard;
        app.vim.mode = Mode::Normal;
        let (text, _) = render(&app);
        // No vim mode label leaks into Standard's status bar.
        assert!(!text.contains("NOR"), "mode chip leaked: {text:?}");
        // But the file/vault tail still renders.
        assert!(text.contains("a.md"), "got: {text:?}");
        assert!(text.contains("theme:"), "got: {text:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn dirty_marker_and_block_count_render() {
        let src = "intro\n\n```db-postgres alias=q connection=cid\nSELECT 1\n```\n";
        let (mut app, _d, _v) = app_with_files(&[("a.md", src)]).await;
        app.config.editor.mode = EditorMode::Vim;
        app.vim.mode = Mode::Normal;
        // Clean → no dirty dot.
        let (clean, _) = render(&app);
        assert!(!clean.contains("·●"), "unexpected dirty dot: {clean:?}");
        assert!(clean.contains("1 blocks"), "got: {clean:?}");
        // Dirty → dot appears.
        app.document_mut().unwrap().mark_dirty();
        let (dirty, _) = render(&app);
        assert!(dirty.contains("·●"), "expected dirty dot: {dirty:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn no_document_falls_back_to_placeholder() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
        app.config.editor.mode = EditorMode::Vim;
        app.vim.mode = Mode::Normal;
        // Drop the document from the active pane so the `(no file)`
        // and `—` cursor fallbacks fire.
        if let Some(p) = app.active_pane_mut() {
            p.document = None;
            p.document_path = None;
        }
        let (text, _) = render(&app);
        assert!(text.contains("(no file)"), "got: {text:?}");
        assert!(text.contains("0 blocks"), "got: {text:?}");
        assert!(text.contains(" — "), "expected cursor dash, got: {text:?}");
    }

    // ---- chips: running / env / conn --------------------------------

    fn running(app: &mut App, segment_idx: usize) {
        app.running_query = Some(RunningQuery {
            segment_idx,
            cancel: CancellationToken::new(),
            started_at: Instant::now(),
            kind: RunningKind::Run,
            cache_key: None,
            bytes_received: 0,
            http_cache_meta: None,
        });
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn running_chip_labels_http_db_and_generic() {
        // One HTTP block + one DB block. Use the canonical JSON body
        // format for HTTP — it's what the parser keys `is_http` off.
        let src = "```http alias=h\n{\"method\":\"GET\",\"url\":\"https://x.com\",\"params\":[],\"headers\":[],\"body\":\"\"}\n```\n\n```db-postgres alias=q connection=cid\nSELECT 1\n```\n";
        let (mut app, _d, _v) = app_with_files(&[("a.md", src)]).await;
        app.config.editor.mode = EditorMode::Vim;
        app.vim.mode = Mode::Normal;

        // Resolve the HTTP block's segment index dynamically.
        let http_idx = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(b) if b.is_http()))
            .unwrap();
        running(&mut app, http_idx);
        assert!(running_chip_label(&app).unwrap().contains("HTTP"));
        let (text, _) = render(&app);
        assert!(text.contains("▶ HTTP ·"), "got: {text:?}");

        // Find the DB block's segment index dynamically.
        let db_idx = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(b) if b.is_db()))
            .unwrap();
        running(&mut app, db_idx);
        assert!(running_chip_label(&app).unwrap().contains("DB"));

        // Out-of-range segment → generic "running" fallback.
        running(&mut app, 999);
        let label = running_chip_label(&app).unwrap();
        assert!(label.contains("running"), "got: {label:?}");

        // No running query → None, no chip.
        app.running_query = None;
        assert!(running_chip_label(&app).is_none());
        let (text, _) = render(&app);
        assert!(!text.contains("▶"), "stale run chip: {text:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn env_chip_present_when_active_env_set() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
        app.config.editor.mode = EditorMode::Vim;
        app.vim.mode = Mode::Normal;

        let (text, _) = render(&app);
        assert!(!text.contains("env:"), "unexpected env chip: {text:?}");

        app.active_env_name = Some("staging".into());
        let (text, buf) = render(&app);
        assert!(text.contains("env: staging"), "got: {text:?}");
        // Magenta background somewhere in the line.
        let has_magenta = (0..200).any(|x| buf.cell((x, 0)).unwrap().bg == Color::LightMagenta);
        assert!(has_magenta, "expected magenta env chip: {text:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn conn_chip_resolves_when_cursor_on_db_block() {
        let src = "intro\n\n```db-postgres alias=q connection=cid-1\nSELECT 1\n```\n";
        let (mut app, _d, _v) = app_with_files(&[("a.md", src)]).await;
        app.config.editor.mode = EditorMode::Vim;
        app.vim.mode = Mode::Normal;
        app.connection_names
            .insert("cid-1".into(), "prod-db".into());

        // Cursor in prose → no conn chip.
        assert!(focused_block_conn_name(&app).is_none());
        let (text, _) = render(&app);
        assert!(!text.contains("conn:"), "unexpected conn chip: {text:?}");

        // Park the cursor inside the DB block.
        let db_idx = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(b) if b.is_db()))
            .unwrap();
        app.document_mut().unwrap().set_cursor(Cursor::InBlock {
            segment_idx: db_idx,
            offset: 0,
        });
        assert_eq!(focused_block_conn_name(&app).as_deref(), Some("prod-db"));
        let (text, buf) = render(&app);
        assert!(text.contains("conn: prod-db"), "got: {text:?}");
        let has_cyan = (0..200).any(|x| buf.cell((x, 0)).unwrap().bg == Color::LightCyan);
        assert!(has_cyan, "expected cyan conn chip: {text:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn conn_chip_absent_for_unresolved_id_and_non_db() {
        let src = "```db-postgres alias=q connection=missing\nSELECT 1\n```\n\n```http alias=h\n{\"method\":\"GET\",\"url\":\"https://x.com\",\"params\":[],\"headers\":[],\"body\":\"\"}\n```\n";
        let (mut app, _d, _v) = app_with_files(&[("a.md", src)]).await;

        // DB block but id not in connection_names → None.
        let db_idx = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(b) if b.is_db()))
            .unwrap();
        app.document_mut().unwrap().set_cursor(Cursor::InBlock {
            segment_idx: db_idx,
            offset: 0,
        });
        assert!(focused_block_conn_name(&app).is_none());

        // Cursor on the HTTP block → not a DB block → None.
        let http_idx = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(b) if b.is_http()))
            .unwrap();
        app.document_mut().unwrap().set_cursor(Cursor::InBlock {
            segment_idx: http_idx,
            offset: 0,
        });
        assert!(focused_block_conn_name(&app).is_none());

        // InBlockResult on the DB block also resolves (covers the
        // second match arm) when the id is known.
        app.connection_names.insert("c2".into(), "named".into());
        let src2 = "```db-postgres alias=q connection=c2\nSELECT 1\n```\n";
        let (mut app2, _d2, _v2) = app_with_files(&[("b.md", src2)]).await;
        app2.connection_names.insert("c2".into(), "named".into());
        let idx2 = app2
            .document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(b) if b.is_db()))
            .unwrap();
        app2.document_mut()
            .unwrap()
            .set_cursor(Cursor::InBlockResult {
                segment_idx: idx2,
                row: 0,
            });
        assert_eq!(focused_block_conn_name(&app2).as_deref(), Some("named"));
    }

    // ---- describe_cursor --------------------------------------------

    #[tokio::test(flavor = "multi_thread")]
    async fn describe_cursor_in_prose_reports_line_col() {
        let (app, _d, _v) = app_with_files(&[("a.md", "line one\nline two\n")]).await;
        let doc = app.document().unwrap();
        // Default cursor lands in prose at offset 0.
        let label = describe_cursor(doc);
        assert!(label.starts_with("Ln 1 Col 1"), "got: {label:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn describe_cursor_in_block_header_body_and_result() {
        let src = "intro\n\n```db-postgres alias=q connection=c\nSELECT 1\nFROM t\n```\n";
        let (mut app, _d, _v) = app_with_files(&[("a.md", src)]).await;
        let db_idx = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(b) if b.is_db()))
            .unwrap();

        // Header (offset 0 → fence line).
        app.document_mut().unwrap().set_cursor(Cursor::InBlock {
            segment_idx: db_idx,
            offset: 0,
        });
        let label = describe_cursor(app.document().unwrap());
        assert!(label.contains("fence ```"), "header: {label:?}");
        assert!(label.starts_with("Block #1"), "header: {label:?}");

        // Body — offset on the first SQL line.
        let raw = match &app.document().unwrap().segments()[db_idx] {
            Segment::Block(b) => b.raw.to_string(),
            _ => unreachable!(),
        };
        let body_off = raw.find("SELECT").unwrap() + 2;
        app.document_mut().unwrap().set_cursor(Cursor::InBlock {
            segment_idx: db_idx,
            offset: body_off,
        });
        let label = describe_cursor(app.document().unwrap());
        assert!(label.contains("Block #1 · Ln "), "body: {label:?}");

        // Closer — offset at the very end of the rope.
        let end = raw.chars().count();
        app.document_mut().unwrap().set_cursor(Cursor::InBlock {
            segment_idx: db_idx,
            offset: end,
        });
        let label = describe_cursor(app.document().unwrap());
        assert!(label.contains("fence ```"), "closer: {label:?}");

        // Result row.
        app.document_mut()
            .unwrap()
            .set_cursor(Cursor::InBlockResult {
                segment_idx: db_idx,
                row: 4,
            });
        let label = describe_cursor(app.document().unwrap());
        assert_eq!(label, "Block #1 · Result row 5");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn describe_cursor_handles_segment_type_mismatch() {
        // Cursor::InProse pointing at a block segment → "Ln ? Col ?".
        let src = "```db-postgres alias=q connection=c\nSELECT 1\n```\n";
        let (mut app, _d, _v) = app_with_files(&[("a.md", src)]).await;
        let db_idx = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(b) if b.is_db()))
            .unwrap();
        app.document_mut().unwrap().set_cursor(Cursor::InProse {
            segment_idx: db_idx,
            offset: 0,
        });
        assert_eq!(describe_cursor(app.document().unwrap()), "Ln ? Col ?");

        // Cursor::InBlock pointing at a prose segment → "Block #N · ?".
        let prose_idx = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Prose(_)))
            .unwrap_or(0);
        if matches!(
            app.document().unwrap().segments().get(prose_idx),
            Some(Segment::Prose(_))
        ) {
            app.document_mut().unwrap().set_cursor(Cursor::InBlock {
                segment_idx: prose_idx,
                offset: 0,
            });
            let label = describe_cursor(app.document().unwrap());
            assert!(label.contains('?'), "got: {label:?}");
        }
    }

    // ---- git chip ----------------------------------------------------

    #[tokio::test(flavor = "multi_thread")]
    async fn git_chip_label_renders_branch_when_in_sync() {
        let (mut app, _d, vault) = app_with_files(&[("a.md", "x\n")]).await;
        crate::git::test_helpers::init_repo(vault.path());
        crate::commands::git::refresh_git_status(&mut app);
        assert_eq!(git_chip_label(&app).as_deref(), Some("main"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn git_chip_label_includes_arrows_when_diverged() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
        app.git_panel.status = Some(httui_core::git::status::GitStatus {
            branch: Some("feature".into()),
            upstream: Some("origin/feature".into()),
            ahead: 2,
            behind: 1,
            changed: vec![],
            clean: true,
        });
        assert_eq!(git_chip_label(&app).as_deref(), Some("feature ↑2 ↓1"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn git_chip_label_none_for_non_git_vault() {
        let (app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
        // Non-git vault → `App::new` populated status_error, status is None.
        assert!(app.git_panel.status.is_none());
        assert!(git_chip_label(&app).is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn status_bar_paints_git_chip_for_git_repo() {
        let (mut app, _d, vault) = app_with_files(&[("a.md", "x\n")]).await;
        crate::git::test_helpers::init_repo(vault.path());
        crate::commands::git::refresh_git_status(&mut app);
        let (text, _) = render(&app);
        assert!(text.contains("main"), "branch chip painted: {text:?}");
    }
}
