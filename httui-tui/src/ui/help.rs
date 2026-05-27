//! Keymap help modal — read-only listing of the chord vocabulary
//! grouped by section. Triggered by `g?` from normal mode; closed
//! by `Esc` / `q` / `Ctrl-C`.
//!
//! V1 is static: the list is hard-coded inside `SECTIONS`, not
//! derived from the parser. That's the right tradeoff at this
//! scale — adding a chord means editing one parser arm and one
//! help entry, both in the same crate. A future iteration could
//! introspect the parser if the surface grows past ~50 chords.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// One row in the help modal: the chord glyph (left column) and a
/// human description (right). Sections collect related rows under
/// a header line.
struct Entry {
    chord: &'static str,
    label: &'static str,
}

struct Section {
    title: &'static str,
    entries: &'static [Entry],
}

/// Hand-curated index of the chord vocabulary the dispatcher
/// understands today. Order is "what the user reaches for first"
/// rather than alphabetical — motions and tabs at the top, modals
/// next, system commands last.
const SECTIONS: &[Section] = &[
    Section {
        title: "Modes",
        entries: &[
            Entry {
                chord: "i / a / I / A",
                label: "enter insert (cursor / after / line-start / line-end)",
            },
            Entry {
                chord: "v / V",
                label: "visual char / visual line",
            },
            Entry {
                chord: "gv",
                label: "reselect last visual region",
            },
            Entry {
                chord: "zz / zt / zb",
                label: "scroll cursor to center / top / bottom",
            },
            Entry {
                chord: "Esc",
                label: "back to normal",
            },
            Entry {
                chord: ":",
                label: "ex command-line",
            },
            Entry {
                chord: "/  ?",
                label: "search forward / backward",
            },
        ],
    },
    Section {
        title: "Files & tabs",
        entries: &[
            Entry {
                chord: "Ctrl-E",
                label: "toggle file tree",
            },
            Entry {
                chord: "Ctrl-P",
                label: "quick-open file (fuzzy)",
            },
            Entry {
                chord: "Ctrl-F",
                label: "content search (FTS5)",
            },
            Entry {
                chord: "gt / gT",
                label: "next / prev tab",
            },
            Entry {
                chord: "<n>gt",
                label: "go to tab n",
            },
            Entry {
                chord: "gb",
                label: "tab picker (centered)",
            },
            Entry {
                chord: "Ctrl-W v / s",
                label: "split vertical / horizontal",
            },
            Entry {
                chord: "Ctrl-W h/j/k/l",
                label: "focus pane left/down/up/right",
            },
            Entry {
                chord: "Ctrl-W c",
                label: "close focused pane",
            },
        ],
    },
    Section {
        title: "Block operations",
        entries: &[
            Entry {
                chord: "Enter",
                label: "run block / open result detail (on result row)",
            },
            Entry {
                chord: "Ctrl-C",
                label: "cancel running query",
            },
            Entry {
                chord: "Ctrl-X",
                label: "EXPLAIN current DB block",
            },
            Entry {
                chord: "gr",
                label: "rerun last block (cursor anywhere)",
            },
            Entry {
                chord: "g] / g[",
                label: "jump to next / previous block",
            },
            Entry {
                chord: "ga",
                label: "edit block alias",
            },
            Entry {
                chord: "gd",
                label: "cycle display mode (input / split / output)",
            },
            Entry {
                chord: "gc",
                label: "open connection picker (DB blocks)",
            },
            Entry {
                chord: "gs",
                label: "open block settings (limit / timeout)",
            },
            Entry {
                chord: "gx",
                label: "open export picker (DB result / HTTP request)",
            },
            Entry {
                chord: "gh",
                label: "open block run-history (HTTP blocks)",
            },
            Entry {
                chord: "Ctrl-Shift-C",
                label: "copy HTTP block as cURL",
            },
            Entry {
                chord: "dd / yy (on block)",
                label: "cut / yank entire block",
            },
        ],
    },
    Section {
        title: "Environments & connections",
        entries: &[
            Entry {
                chord: "gE",
                label: "open environment picker",
            },
            Entry {
                chord: "gc",
                label: "open connection picker (in DB block)",
            },
            Entry {
                chord: "D (in conn picker)",
                label: "delete highlighted connection",
            },
        ],
    },
    Section {
        title: "Help & misc",
        entries: &[
            Entry {
                chord: "g?",
                label: "this help modal",
            },
            Entry {
                chord: "gN",
                label: "insert new block from template",
            },
            Entry {
                chord: ":w / :wq / :q / :q!",
                label: "write / write-quit / quit / force-quit",
            },
            Entry {
                chord: "Ctrl-S",
                label: "save (works in normal + insert)",
            },
            Entry {
                chord: "gW",
                label: "write all dirty tabs",
            },
            Entry {
                chord: ":e <path>",
                label: "open file",
            },
            Entry {
                chord: ":%s/foo/bar",
                label: "substitute (literal, doc-global)",
            },
            Entry {
                chord: ":N",
                label: "go to line N (e.g., :42)",
            },
            Entry {
                chord: "u / Ctrl-R",
                label: "undo / redo",
            },
            Entry {
                chord: "Y (in detail modal)",
                label: "copy entire body to clipboard",
            },
        ],
    },
];

pub fn render(frame: &mut Frame, editor_area: Rect) {
    let popup = compute_popup_rect(editor_area);
    let bg_style = Style::default()
        .bg(crate::ui::palette::popup_bg())
        .fg(crate::ui::palette::foreground());

    // Hard-fill the popup area before painting so editor content
    // underneath doesn't bleed through.
    {
        let buf = frame.buffer_mut();
        for y in popup.y..popup.y.saturating_add(popup.height) {
            for x in popup.x..popup.x.saturating_add(popup.width) {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol(" ");
                    cell.set_style(bg_style);
                }
            }
        }
    }

    let outer = Block::default()
        .borders(Borders::ALL)
        .title(" Keymap help · g? ")
        .style(bg_style)
        .border_style(
            Style::default()
                .fg(Color::LightCyan)
                .bg(crate::ui::palette::popup_bg()),
        );
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);
    let body_area = chunks[0];
    let footer_area = chunks[1];

    // Pre-compute the chord-column width so descriptions line up
    // across sections. Add a 2-cell gutter between chord and label.
    let chord_width = SECTIONS
        .iter()
        .flat_map(|s| s.entries.iter())
        .map(|e| e.chord.chars().count())
        .max()
        .unwrap_or(8);

    let mut lines: Vec<Line> = Vec::new();
    for (idx, section) in SECTIONS.iter().enumerate() {
        if idx > 0 {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(Span::styled(
            format!("  {}", section.title),
            Style::default()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
        )));
        for entry in section.entries {
            let pad = chord_width.saturating_sub(entry.chord.chars().count());
            let chord_cell = format!("    {}{:>pad$}  ", entry.chord, "", pad = pad);
            lines.push(Line::from(vec![
                Span::styled(
                    chord_cell,
                    Style::default()
                        .fg(crate::ui::palette::popup_border_accent())
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    entry.label,
                    Style::default().fg(crate::ui::palette::foreground()),
                ),
            ]));
        }
    }
    frame.render_widget(Paragraph::new(lines).style(bg_style), body_area);

    let chip_key = Style::default()
        .bg(Color::LightCyan)
        .fg(crate::ui::palette::popup_bg())
        .add_modifier(Modifier::BOLD);
    let chip_label = Style::default().fg(Color::Gray);
    let footer = Line::from(vec![
        Span::styled(" Esc / q ", chip_key),
        Span::styled(" close ", chip_label),
    ]);
    frame.render_widget(Paragraph::new(footer).style(bg_style), footer_area);
}

/// Compute the popup rect: a roomy ~70×... box, centered horizontally,
/// floated 2 rows below the editor top. Width is 70 (or as much as
/// the editor allows); height fits all sections + chrome but caps at
/// editor_height - 4.
fn compute_popup_rect(area: Rect) -> Rect {
    let lines: usize = SECTIONS.iter().map(|s| s.entries.len() + 2).sum::<usize>() + 1;
    let desired_height = (lines as u16).saturating_add(3);
    let max_height = area.height.saturating_sub(4);
    let height = desired_height.min(max_height).max(10);
    let width = 72u16.min(area.width.saturating_sub(2));
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + 2u16.min(area.height.saturating_sub(height));
    Rect {
        x,
        y,
        width,
        height,
    }
}
