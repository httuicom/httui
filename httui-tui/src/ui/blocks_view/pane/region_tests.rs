use super::*;

#[test]
fn multiline_edit_pans_to_keep_caret_visible() {
    use ratatui::backend::{Backend, TestBackend};
    use ratatui::Terminal;
    let sql = format!("SELECT {} AS TAILCOL", "x".repeat(60));
    let mut doc = crate::buffer::Document::from_markdown(&format!("{sql}\n")).unwrap();
    // Caret on the line's last char → the stateless pan must bring
    // the tail into a 40-col region.
    doc.set_cursor(crate::buffer::Cursor::InProse {
        segment_idx: 0,
        offset: sql.chars().count() - 1,
    });
    let mut pane = crate::pane::Pane::empty();
    pane.block_edit = Some(Box::new(crate::app::RegionEdit {
        field: EditField::DbQuery,
        doc,
        sub_mode: crate::app::EditSubMode::Insert,
    }));
    let backend = TestBackend::new(40, 4);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            render_multiline_region(
                f,
                Rect::new(0, 0, 40, 4),
                "db-postgres",
                true,
                "",
                "(no query)",
                &pane,
                true,
                |f| matches!(f, EditField::DbQuery),
                None,
            );
        })
        .unwrap();
    let cur = terminal.backend_mut().get_cursor_position().unwrap();
    let buf = terminal.backend().buffer().clone();
    let text: String = (0..4)
        .flat_map(|y| (0..40).map(move |x| (x, y)))
        .map(|(x, y)| buf.cell((x, y)).unwrap().symbol().to_string())
        .collect();
    assert!(text.contains("TAILCOL"), "tail visible under pan: {text:?}");
    assert!(cur.x < 40, "caret inside the region, got {}", cur.x);
}

#[test]
fn multiline_edit_scrolls_vertically_to_the_caret_row() {
    use ratatui::backend::{Backend, TestBackend};
    use ratatui::Terminal;
    // 30-line body, caret on the last line — a 4-row region must
    // scroll down to show it instead of clipping at the top.
    let body: String = (0..29).map(|i| format!("line{i}\n")).collect::<String>() + "SELECT TAILROW";
    let mut doc = crate::buffer::Document::from_markdown(&format!("{body}\n")).unwrap();
    doc.set_cursor(crate::buffer::Cursor::InProse {
        segment_idx: 0,
        offset: body.chars().count() - 1,
    });
    let mut pane = crate::pane::Pane::empty();
    pane.block_edit = Some(Box::new(crate::app::RegionEdit {
        field: EditField::DbQuery,
        doc,
        sub_mode: crate::app::EditSubMode::Insert,
    }));
    let backend = TestBackend::new(40, 4);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            render_multiline_region(
                f,
                Rect::new(0, 0, 40, 4),
                "db-postgres",
                true,
                "",
                "(no query)",
                &pane,
                true,
                |f| matches!(f, EditField::DbQuery),
                None,
            );
        })
        .unwrap();
    let cur = terminal.backend_mut().get_cursor_position().unwrap();
    let buf = terminal.backend().buffer().clone();
    let text: String = (0..4)
        .flat_map(|y| (0..40).map(move |x| (x, y)))
        .map(|(x, y)| buf.cell((x, y)).unwrap().symbol().to_string())
        .collect();
    assert!(text.contains("TAILROW"), "last line visible: {text:?}");
    assert!(!text.contains("line0"), "top scrolled off: {text:?}");
    assert!(cur.y < 4, "caret row inside the region, got {}", cur.y);
}

#[test]
fn multiline_json_body_is_syntax_highlighted() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    let body = "{\n  \"name\": \"joao\"\n}";
    let mut doc = crate::buffer::Document::from_markdown(&format!("{body}\n")).unwrap();
    doc.set_cursor(crate::buffer::Cursor::InProse {
        segment_idx: 0,
        offset: 0,
    });
    let mut pane = crate::pane::Pane::empty();
    pane.block_edit = Some(Box::new(crate::app::RegionEdit {
        field: EditField::HttpBody,
        doc,
        sub_mode: crate::app::EditSubMode::Insert,
    }));
    let backend = TestBackend::new(30, 5);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            render_multiline_region(
                f,
                Rect::new(0, 0, 30, 5),
                "http",
                true,
                "",
                "(no body)",
                &pane,
                true,
                |f| matches!(f, EditField::HttpBody),
                None,
            );
        })
        .unwrap();
    let buf = terminal.backend().buffer().clone();
    // Text stays verbatim on screen…
    let text: String = (0..5)
        .flat_map(|y| (0..30).map(move |x| (x, y)))
        .map(|(x, y)| buf.cell((x, y)).unwrap().symbol().to_string())
        .collect();
    assert!(text.contains("\"name\""), "verbatim JSON: {text:?}");
    // …and at least one cell carries the JSON key colour (cyan).
    let has_colour = (0..5)
        .flat_map(|y| (0..30).map(move |x| (x, y)))
        .any(|(x, y)| buf.cell((x, y)).unwrap().fg == ratatui::style::Color::Cyan);
    assert!(has_colour, "JSON keys must be coloured");
}

#[test]
fn multiline_nav_mode_renders_unpanned() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    let pane = crate::pane::Pane::empty();
    let backend = TestBackend::new(40, 3);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            render_multiline_region(
                f,
                Rect::new(0, 0, 40, 3),
                "db-postgres",
                false,
                "SELECT HEADCOL FROM t",
                "(no query)",
                &pane,
                true,
                |f| matches!(f, EditField::DbQuery),
                None,
            );
        })
        .unwrap();
    let buf = terminal.backend().buffer().clone();
    let text: String = (0..3)
        .flat_map(|y| (0..40).map(move |x| (x, y)))
        .map(|(x, y)| buf.cell((x, y)).unwrap().symbol().to_string())
        .collect();
    assert!(text.contains("HEADCOL"), "NAV stays unpanned: {text:?}");
}

#[test]
fn refs_spans_chips_a_reference() {
    let spans = refs_spans("/users/{{id}}", false);
    let chip = spans
        .iter()
        .find(|s| s.content == "{{id}}")
        .expect("ref chip span");
    assert_eq!(chip.style, crate::ui::blocks::ref_highlight::normal_style());
}

#[test]
fn refs_spans_plain_text_is_a_single_span() {
    let spans = refs_spans("no refs here", false);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].content, "no refs here");
}

#[test]
fn refs_spans_underlines_every_span_when_focused() {
    let spans = refs_spans("a {{b}} c", true);
    assert!(!spans.is_empty());
    assert!(spans
        .iter()
        .all(|s| s.style.add_modifier.contains(Modifier::UNDERLINED)));
}
