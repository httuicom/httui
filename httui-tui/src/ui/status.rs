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
        let line = Line::from(vec![Span::raw(format!(":{}", app.vim.cmdline.as_str()))]);
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
            };
            let line = Line::from(vec![
                Span::styled(label, Style::default().fg(Color::LightYellow)),
                Span::raw(prompt.buffer().to_string()),
            ]);
            frame.render_widget(Paragraph::new(line), area);
        }
        return;
    }

    if app.vim.mode == Mode::Search {
        let prompt = if app.vim.search_forward { '/' } else { '?' };
        let line = Line::from(vec![Span::raw(format!(
            "{prompt}{}",
            app.vim.search_buf.as_str()
        ))]);
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
                .fg(Color::White)
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
                    .fg(Color::Black)
                    .bg(Color::LightGreen)
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
                    .fg(Color::Black)
                    .bg(Color::LightMagenta)
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
                    .fg(Color::Black)
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
                .fg(Color::Black)
                .bg(mode.bg())
                .add_modifier(Modifier::BOLD),
        )]
    } else {
        Vec::new()
    };
    spans.extend(running_chip);
    spans.extend(env_chip);
    spans.extend(conn_chip);
    spans.push(Span::raw(format!(
        " {file}{dirty_marker} · {block_count} blocks · {cursor_label} · vault: {vault} · theme: {}",
        app.config.theme
    )));
    let line = Line::from(spans);
    frame.render_widget(Paragraph::new(line), area);
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

/// Build the running-indicator chip label, or `None` when no
/// query / HTTP request is in flight. Format: `▶ DB · 2.3s` or
/// `▶ HTTP · 1.1s`. Block-type comes from the source block at
/// `RunningQuery.segment_idx`; if the document has shifted under
/// the in-flight task and the segment isn't a block anymore, we
/// fall back to a generic `▶ running · Xs` so the user still sees
/// "something is happening".
fn running_chip_label(app: &App) -> Option<String> {
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
    Some(format!("▶ {kind} · {elapsed:.1}s"))
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
    use super::compact_vault_path;
    use std::path::PathBuf;

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
}
