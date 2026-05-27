//! Connections page (V3, 2026-05-23 + polish 2026-05-23). Master-
//! detail popup listing every entry in `<vault>/connections.toml`.
//! Polish pass aproxima a UX do desktop: chip colorido por driver,
//! detail agrupado em sections, popup denso (não fullscreen).
//! Triggered by `gC` / `Alt+P`. `n` opens the create form; `e`/`D`
//! land in P3/P4.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use crate::app::{ConnectionDetail, ConnectionsPageState};
use crate::schema::SchemaCache;

const POPUP_WIDTH: u16 = 64;
/// Tall enough for the full detail pane (header + Connection + Auth +
/// Options + Description + Used in up to 6 refs) without clipping
/// on a default 24-row terminal it still leaves a margin around it.
const POPUP_HEIGHT: u16 = 32;
const SIDEBAR_COLS: u16 = 22;

pub fn render(
    frame: &mut Frame,
    editor_area: Rect,
    state: &ConnectionsPageState,
    schema_cache: &SchemaCache,
    session_overrides: &crate::session_overrides::ConnectionOverrideStore,
) {
    let area = centered_rect(editor_area);
    let bg_style = Style::default()
        .bg(crate::ui::palette::popup_bg())
        .fg(crate::ui::palette::foreground());

    // Hard-fill so editor content underneath doesn't bleed through.
    {
        let buf = frame.buffer_mut();
        for y in area.y..area.y.saturating_add(area.height) {
            for x in area.x..area.x.saturating_add(area.width) {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol(" ");
                    cell.set_style(bg_style);
                }
            }
        }
    }

    let title = format!(" Connections · {} ", state.connections.len());
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(bg_style)
        .border_style(
            Style::default()
                .fg(Color::LightBlue)
                .bg(crate::ui::palette::popup_bg()),
        );
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    // Vertical: body | hint footer.
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);

    // Body: sidebar | divider | detail.
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(SIDEBAR_COLS),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(rows[0]);

    render_sidebar(frame, body[0], state, session_overrides, bg_style);
    render_divider(frame, body[1], bg_style);
    render_detail(
        frame,
        body[2],
        state,
        schema_cache,
        session_overrides,
        bg_style,
    );
    render_hint(frame, rows[1], bg_style);
}

fn render_sidebar(
    frame: &mut Frame,
    area: Rect,
    state: &ConnectionsPageState,
    overrides: &crate::session_overrides::ConnectionOverrideStore,
    bg: Style,
) {
    if state.connections.is_empty() {
        let empty = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                " no conns yet",
                Style::default().fg(crate::ui::palette::muted()),
            )),
            Line::from(""),
            Line::from(Span::styled(
                " press n to add",
                Style::default()
                    .fg(crate::ui::palette::popup_border_accent())
                    .add_modifier(Modifier::ITALIC),
            )),
        ])
        .style(bg)
        .wrap(Wrap { trim: false });
        frame.render_widget(empty, area);
        return;
    }

    let items: Vec<ListItem> = state
        .connections
        .iter()
        .map(|c| {
            let (chip_label, chip_color) = driver_chip(&c.driver);
            let mut spans = vec![
                Span::raw(" "),
                Span::styled(
                    format!(" {chip_label} "),
                    Style::default()
                        .bg(chip_color)
                        .fg(crate::ui::palette::popup_bg())
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    c.name.clone(),
                    Style::default().fg(crate::ui::palette::foreground()),
                ),
            ];
            if overrides.is_active(&c.name) {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    " TEMP ",
                    Style::default()
                        .bg(crate::ui::palette::amber())
                        .fg(crate::ui::palette::amber_fg_on_amber_bg())
                        .add_modifier(Modifier::BOLD),
                ));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items)
        .style(bg)
        .highlight_style(
            Style::default()
                .bg(crate::ui::palette::selection_bg())
                .fg(crate::ui::palette::foreground())
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▌");

    let mut list_state = ListState::default();
    list_state.select(Some(state.selected));
    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_divider(frame: &mut Frame, area: Rect, _bg: Style) {
    let buf = frame.buffer_mut();
    for y in area.y..area.y.saturating_add(area.height) {
        if let Some(cell) = buf.cell_mut((area.x, y)) {
            cell.set_symbol("│");
            cell.set_style(
                Style::default()
                    .fg(crate::ui::palette::muted())
                    .bg(crate::ui::palette::popup_bg()),
            );
        }
    }
}

fn render_detail(
    frame: &mut Frame,
    area: Rect,
    state: &ConnectionsPageState,
    schema_cache: &SchemaCache,
    overrides: &crate::session_overrides::ConnectionOverrideStore,
    bg: Style,
) {
    let Some(detail) = state.connections.get(state.selected) else {
        let empty =
            Paragraph::new("  (no connection selected)").style(bg.fg(crate::ui::palette::muted()));
        frame.render_widget(empty, area);
        return;
    };

    let mut lines = detail_lines(detail);
    // Always rendered (even empty) so the `(none)` state and the
    // chord affordance stay discoverable.
    lines.extend(session_override_lines(&detail.name, overrides));
    // V3 P5.1: vault-grep refs.
    lines.extend(used_in_lines(&state.uses));
    // V3 P5.2: schema preview — sync read of the in-memory cache;
    // shows "loading…" when the background introspection hasn't
    // landed yet.
    lines.extend(schema_lines(&detail.name, schema_cache));
    let para = Paragraph::new(lines).style(bg).wrap(Wrap { trim: false });
    let inner = Rect {
        x: area.x.saturating_add(2),
        y: area.y,
        width: area.width.saturating_sub(2),
        height: area.height,
    };
    frame.render_widget(para, inner);
}

fn schema_lines(connection_name: &str, schema_cache: &SchemaCache) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from("")];
    match schema_cache.get(connection_name) {
        None => {
            lines.push(section_header("Schema"));
            lines.push(Line::from(Span::styled(
                "loading…",
                Style::default()
                    .fg(crate::ui::palette::muted())
                    .add_modifier(Modifier::ITALIC),
            )));
        }
        Some(entry) if entry.tables.is_empty() => {
            lines.push(section_header("Schema · 0 tables"));
            lines.push(Line::from(Span::styled(
                "(empty)",
                Style::default()
                    .fg(crate::ui::palette::muted())
                    .add_modifier(Modifier::ITALIC),
            )));
        }
        Some(entry) => {
            let total_cols: usize = entry.tables.iter().map(|t| t.columns.len()).sum();
            lines.push(section_header(&format!(
                "Schema · {} tables · {} cols",
                entry.tables.len(),
                total_cols
            )));
            const MAX_ROWS: usize = 5;
            for t in entry.tables.iter().take(MAX_ROWS) {
                let qualified = match &t.schema {
                    Some(s) => format!("{s}.{}", t.name),
                    None => t.name.clone(),
                };
                lines.push(Line::from(vec![
                    Span::styled(
                        qualified,
                        Style::default().fg(crate::ui::palette::foreground()),
                    ),
                    Span::styled(
                        format!(" ({} cols)", t.columns.len()),
                        Style::default().fg(crate::ui::palette::muted()),
                    ),
                ]));
            }
            if entry.tables.len() > MAX_ROWS {
                lines.push(Line::from(Span::styled(
                    format!("+{} more", entry.tables.len() - MAX_ROWS),
                    Style::default()
                        .fg(crate::ui::palette::muted())
                        .add_modifier(Modifier::ITALIC),
                )));
            }
        }
    }
    lines
}

fn used_in_lines(uses: &[crate::app::ConnectionUse]) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(""),
        section_header(&format!("Used in {}", uses.len())),
    ];
    if uses.is_empty() {
        lines.push(Line::from(Span::styled(
            "(no references in this vault)",
            Style::default()
                .fg(crate::ui::palette::muted())
                .add_modifier(Modifier::ITALIC),
        )));
    } else {
        // Cap at 6 rows so the section doesn't push everything below
        // off-screen on a 20-row popup. Surplus surfaces as a "+N more".
        const MAX_ROWS: usize = 6;
        for u in uses.iter().take(MAX_ROWS) {
            lines.push(Line::from(vec![
                Span::styled(
                    u.file.clone(),
                    Style::default().fg(crate::ui::palette::foreground()),
                ),
                Span::styled(
                    format!(":{}", u.line),
                    Style::default().fg(crate::ui::palette::muted()),
                ),
            ]));
        }
        if uses.len() > MAX_ROWS {
            lines.push(Line::from(Span::styled(
                format!("+{} more", uses.len() - MAX_ROWS),
                Style::default()
                    .fg(crate::ui::palette::muted())
                    .add_modifier(Modifier::ITALIC),
            )));
        }
    }
    lines
}

fn driver_chip(driver: &str) -> (&'static str, Color) {
    match driver {
        "postgres" => ("PG", crate::ui::palette::popup_key_label()),
        "mysql" => ("MY", crate::ui::palette::popup_border_accent()),
        "sqlite" => ("SL", crate::ui::palette::success()),
        _ => ("??", crate::ui::palette::muted()),
    }
}

fn detail_lines(c: &ConnectionDetail) -> Vec<Line<'static>> {
    let label = |text: &str| {
        Span::styled(
            format!("{text:<10}"),
            Style::default().fg(crate::ui::palette::muted()),
        )
    };
    let value =
        |text: String| Span::styled(text, Style::default().fg(crate::ui::palette::foreground()));
    let none = || Span::styled("—", Style::default().fg(crate::ui::palette::muted()));
    let opt = |s: &Option<String>| -> Span<'static> {
        s.as_ref().map(|v| value(v.clone())).unwrap_or_else(none)
    };
    let opt_port =
        |p: Option<u16>| -> Span<'static> { p.map(|v| value(v.to_string())).unwrap_or_else(none) };
    let (chip_label, chip_color) = driver_chip(&c.driver);

    let mut lines = vec![
        // Header row: name + driver chip.
        Line::from(vec![
            Span::styled(
                c.name.clone(),
                Style::default()
                    .fg(Color::LightCyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                format!(" {chip_label} "),
                Style::default()
                    .bg(chip_color)
                    .fg(crate::ui::palette::popup_bg())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {}", c.driver),
                Style::default().fg(crate::ui::palette::muted()),
            ),
        ]),
        Line::from(""),
        // Section: Connection.
        section_header("Connection"),
        Line::from(vec![label("host"), opt(&c.host)]),
        Line::from(vec![label("port"), opt_port(c.port)]),
        Line::from(vec![label("database"), opt(&c.database_name)]),
        Line::from(""),
        // Section: Auth.
        section_header("Auth"),
        Line::from(vec![label("username"), opt(&c.username)]),
        Line::from(vec![
            label("password"),
            if c.has_password {
                Span::styled(
                    "•••• (keychain)",
                    Style::default().fg(crate::ui::palette::foreground()),
                )
            } else {
                none()
            },
        ]),
        Line::from(""),
        // Section: Options.
        section_header("Options"),
        Line::from(vec![label("ssl_mode"), opt(&c.ssl_mode)]),
        Line::from(vec![
            label("readonly"),
            if c.is_readonly {
                Span::styled(
                    "yes",
                    Style::default()
                        .fg(crate::ui::palette::popup_border_accent())
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled("no", Style::default().fg(crate::ui::palette::foreground()))
            },
        ]),
    ];
    if let Some(desc) = c.description.as_deref() {
        lines.push(Line::from(""));
        lines.push(section_header("Description"));
        lines.push(Line::from(Span::styled(
            desc.to_string(),
            Style::default().fg(crate::ui::palette::foreground()),
        )));
    }
    lines
}

fn section_header(title: &str) -> Line<'static> {
    Line::from(vec![Span::styled(
        format!("── {title} "),
        Style::default()
            .fg(Color::LightBlue)
            .add_modifier(Modifier::BOLD),
    )])
}

fn centered_rect(area: Rect) -> Rect {
    let w = POPUP_WIDTH.min(area.width.saturating_sub(2));
    let h = POPUP_HEIGHT.min(area.height.saturating_sub(2));
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

fn render_hint(frame: &mut Frame, area: Rect, _bg: Style) {
    let hint = " jk nav · n/e/t/D · o set · O clear · Esc close ";
    let para = Paragraph::new(Span::styled(
        hint,
        Style::default()
            .fg(crate::ui::palette::muted())
            .bg(crate::ui::palette::popup_bg())
            .add_modifier(Modifier::ITALIC),
    ));
    frame.render_widget(para, area);
}

/// Header is always rendered (even with no override set) for
/// affordance discoverability. Active values are amber so the
/// non-persistent state pops at a glance.
fn session_override_lines(
    connection_name: &str,
    overrides: &crate::session_overrides::ConnectionOverrideStore,
) -> Vec<Line<'static>> {
    let amber = crate::ui::palette::amber();
    let header = Line::from(Span::styled(
        "── Session override (TEMP) ",
        Style::default().fg(amber).add_modifier(Modifier::BOLD),
    ));
    let mut out = vec![Line::from(""), header];
    match overrides.get(connection_name) {
        Some(ov) if !ov.is_empty() => {
            let host = ov.host.as_deref().unwrap_or("—");
            let port = ov.port.map(|p| p.to_string()).unwrap_or_else(|| "—".into());
            out.push(Line::from(vec![
                Span::styled(
                    format!("{:<10}", "host"),
                    Style::default().fg(crate::ui::palette::muted()),
                ),
                Span::styled(
                    host.to_string(),
                    Style::default().fg(amber).add_modifier(Modifier::BOLD),
                ),
            ]));
            out.push(Line::from(vec![
                Span::styled(
                    format!("{:<10}", "port"),
                    Style::default().fg(crate::ui::palette::muted()),
                ),
                Span::styled(
                    port,
                    Style::default().fg(amber).add_modifier(Modifier::BOLD),
                ),
            ]));
        }
        _ => {
            out.push(Line::from(Span::styled(
                "(none) — press `o` to set, `O` to clear",
                Style::default()
                    .fg(crate::ui::palette::muted())
                    .add_modifier(Modifier::ITALIC),
            )));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn detail(name: &str, driver: &str) -> ConnectionDetail {
        ConnectionDetail {
            name: name.into(),
            driver: driver.into(),
            host: Some("localhost".into()),
            port: Some(5432),
            database_name: Some("mydb".into()),
            username: Some("user".into()),
            has_password: true,
            ssl_mode: None,
            is_readonly: false,
            description: None,
        }
    }

    fn render_page(state: &ConnectionsPageState, w: u16, h: u16) -> String {
        render_page_with_cache(state, &SchemaCache::new(), w, h)
    }

    fn render_page_with_cache(
        state: &ConnectionsPageState,
        cache: &SchemaCache,
        w: u16,
        h: u16,
    ) -> String {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        let overrides = crate::session_overrides::ConnectionOverrideStore::default();
        terminal
            .draw(|f| {
                render(f, Rect::new(0, 0, w, h), state, cache, &overrides);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        (0..h)
            .map(|y| {
                let line: String = (0..w)
                    .map(|x| buf.cell((x, y)).unwrap().symbol().to_string())
                    .collect();
                line.trim_end().to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn render_empty_state_shows_hint() {
        let state = ConnectionsPageState {
            connections: Vec::new(),
            selected: 0,
            ..Default::default()
        };
        let text = render_page(&state, 80, 24);
        assert!(text.contains("Connections · 0"));
        assert!(text.contains("no conns yet"));
        assert!(text.contains("press n to add"));
    }

    #[test]
    fn render_populated_list_paints_entries_and_chips() {
        let state = ConnectionsPageState {
            connections: vec![detail("Test", "sqlite"), detail("old-htui", "postgres")],
            selected: 0,
            ..Default::default()
        };
        let text = render_page(&state, 80, 24);
        assert!(text.contains("Connections · 2"));
        assert!(text.contains("Test"));
        assert!(text.contains("old-htui"));
        // Chip labels (SL/PG) appear in the sidebar.
        assert!(text.contains("SL"), "sqlite chip missing: {text}");
        assert!(text.contains("PG"), "postgres chip missing: {text}");
    }

    #[test]
    fn render_detail_pane_groups_into_sections() {
        let state = ConnectionsPageState {
            connections: vec![detail("Test", "sqlite")],
            selected: 0,
            ..Default::default()
        };
        let text = render_page(&state, 80, 24);
        assert!(text.contains("Connection"));
        assert!(text.contains("Auth"));
        assert!(text.contains("Options"));
        // Specific value lines.
        assert!(text.contains("localhost"));
        assert!(text.contains("mydb"));
        assert!(text.contains("user"));
        assert!(text.contains("•••• (keychain)"));
    }

    #[test]
    fn render_includes_description_section_when_present() {
        let mut d = detail("Test", "sqlite");
        d.description = Some("local dev db".into());
        let state = ConnectionsPageState {
            connections: vec![d],
            selected: 0,
            ..Default::default()
        };
        let text = render_page(&state, 80, 24);
        assert!(text.contains("Description"));
        assert!(text.contains("local dev db"));
    }

    #[test]
    fn render_omits_description_when_absent() {
        let state = ConnectionsPageState {
            connections: vec![detail("Test", "sqlite")],
            selected: 0,
            ..Default::default()
        };
        let text = render_page(&state, 80, 24);
        assert!(!text.contains("Description"));
    }

    #[test]
    fn render_readonly_yes_when_flagged() {
        let mut d = detail("Test", "sqlite");
        d.is_readonly = true;
        let state = ConnectionsPageState {
            connections: vec![d],
            selected: 0,
            ..Default::default()
        };
        let text = render_page(&state, 80, 24);
        assert!(text.contains("yes"));
    }

    #[test]
    fn render_footer_hint_lists_chords() {
        let state = ConnectionsPageState {
            connections: vec![detail("Test", "sqlite")],
            selected: 0,
            ..Default::default()
        };
        let text = render_page(&state, 80, 24);
        assert!(text.contains("jk nav"));
        assert!(text.contains("Esc close"));
        assert!(text.contains("o set"));
        assert!(text.contains("O clear"));
    }

    #[test]
    fn driver_chip_unknown_driver_falls_back() {
        let (label, _) = driver_chip("oracle");
        assert_eq!(label, "??");
    }

    #[test]
    fn render_smoke_does_not_panic_on_small_area() {
        let state = ConnectionsPageState {
            connections: vec![detail("a", "sqlite")],
            selected: 0,
            ..Default::default()
        };
        let _ = render_page(&state, 30, 10);
    }

    #[test]
    fn render_used_in_shows_no_references_when_empty() {
        let state = ConnectionsPageState {
            connections: vec![detail("a", "sqlite")],
            selected: 0,
            uses: Vec::new(),
        };
        let text = render_page(&state, 80, 30);
        assert!(text.contains("Used in 0"));
        assert!(text.contains("(no references in this vault)"));
    }

    #[test]
    fn render_used_in_lists_file_line_refs() {
        let state = ConnectionsPageState {
            connections: vec![detail("Test", "sqlite")],
            selected: 0,
            uses: vec![
                crate::app::ConnectionUse {
                    file: "runbooks/users.md".into(),
                    line: 12,
                },
                crate::app::ConnectionUse {
                    file: "scratch.md".into(),
                    line: 3,
                },
            ],
        };
        let text = render_page(&state, 80, 30);
        assert!(text.contains("Used in 2"));
        assert!(text.contains("runbooks/users.md"));
        assert!(text.contains(":12"));
        assert!(text.contains("scratch.md"));
        assert!(text.contains(":3"));
    }

    #[test]
    fn render_used_in_caps_long_lists_with_more_marker() {
        let uses: Vec<_> = (1..=10)
            .map(|i| crate::app::ConnectionUse {
                file: format!("note-{i}.md"),
                line: i as u32,
            })
            .collect();
        let state = ConnectionsPageState {
            connections: vec![detail("a", "sqlite")],
            selected: 0,
            uses,
        };
        let text = render_page(&state, 80, 40);
        assert!(text.contains("Used in 10"));
        assert!(text.contains("note-1.md"));
        assert!(text.contains("note-6.md"));
        assert!(text.contains("+4 more"));
    }

    // ---------- V3 P5.2: schema preview ----------

    #[test]
    fn schema_section_shows_loading_when_cache_miss() {
        let state = ConnectionsPageState {
            connections: vec![detail("Test", "sqlite")],
            selected: 0,
            ..Default::default()
        };
        let text = render_page_with_cache(&state, &SchemaCache::new(), 80, 36);
        assert!(text.contains("Schema"));
        assert!(text.contains("loading"));
    }

    #[test]
    fn schema_section_shows_empty_marker_when_no_tables() {
        let mut cache = SchemaCache::new();
        cache.store("Test", Vec::new());
        let state = ConnectionsPageState {
            connections: vec![detail("Test", "sqlite")],
            selected: 0,
            ..Default::default()
        };
        let text = render_page_with_cache(&state, &cache, 80, 36);
        assert!(text.contains("Schema · 0 tables"));
        assert!(text.contains("(empty)"));
    }

    #[test]
    fn schema_section_lists_tables_with_col_counts() {
        use crate::schema::{SchemaColumn, SchemaTable};
        let mut cache = SchemaCache::new();
        cache.store(
            "Test",
            vec![
                SchemaTable {
                    schema: None,
                    name: "users".into(),
                    columns: vec![
                        SchemaColumn {
                            name: "id".into(),
                            data_type: Some("integer".into()),
                        },
                        SchemaColumn {
                            name: "name".into(),
                            data_type: Some("text".into()),
                        },
                    ],
                },
                SchemaTable {
                    schema: Some("public".into()),
                    name: "orders".into(),
                    columns: vec![SchemaColumn {
                        name: "id".into(),
                        data_type: Some("integer".into()),
                    }],
                },
            ],
        );
        let state = ConnectionsPageState {
            connections: vec![detail("Test", "sqlite")],
            selected: 0,
            ..Default::default()
        };
        let text = render_page_with_cache(&state, &cache, 80, 36);
        assert!(text.contains("Schema · 2 tables · 3 cols"));
        assert!(text.contains("users"));
        assert!(text.contains("(2 cols)"));
        // Qualified table name with schema prefix.
        assert!(text.contains("public.orders"));
        assert!(text.contains("(1 cols)"));
    }

    #[test]
    fn schema_section_caps_with_more_marker() {
        use crate::schema::SchemaTable;
        let mut cache = SchemaCache::new();
        let tables: Vec<_> = (1..=10)
            .map(|i| SchemaTable {
                schema: None,
                name: format!("t{i}"),
                columns: vec![],
            })
            .collect();
        cache.store("Test", tables);
        let state = ConnectionsPageState {
            connections: vec![detail("Test", "sqlite")],
            selected: 0,
            ..Default::default()
        };
        let text = render_page_with_cache(&state, &cache, 80, 40);
        assert!(text.contains("Schema · 10 tables"));
        assert!(text.contains("t1 "));
        assert!(text.contains("t5 "));
        assert!(text.contains("+5 more"));
    }
}
