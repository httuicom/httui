// size:exclude file — render_root tests cluster (extracted from render_root.rs)
use super::*;
use crate::app::{App, BlockExportFormat, CompletionPopupState};
use crate::app::{
    BlockHistoryState, BlockTemplatePickerState, ConnectionPickerState, ContentSearchState,
    DbConfirmRunState, DbExportPickerState, DbRowDetailState, DbSettingsState,
    EnvironmentPickerState, HttpResponseDetailState, SettingsField, TabPickerState,
};
use crate::buffer::{Cursor, Document};
use crate::config::{Config, EditorMode};
use crate::pane::{Pane, TabState};
use crate::vault::ResolvedVault;
use httui_core::db::init_db;
use ratatui::backend::{Backend, TestBackend};
use ratatui::Terminal;
use std::path::PathBuf;
use tempfile::TempDir;

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

/// Open `a.md` into a focused leaf of tab 0 so the renderer has a
/// real document tree to walk instead of the empty-state path.
fn open_doc(app: &mut App, md: &str) {
    let doc = Document::from_markdown(md).unwrap();
    let pane = Pane::new(doc, PathBuf::from("a.md"));
    app.tabs.tabs = vec![TabState::new(pane)];
    app.tabs.active = 0;
}

fn render(app: &mut App, w: u16, h: u16) -> (String, Option<(u16, u16)>) {
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            super::render(f, app);
        })
        .unwrap();
    let cur = terminal
        .backend_mut()
        .get_cursor_position()
        .ok()
        .map(|p| (p.x, p.y));
    let buf = terminal.backend().buffer().clone();
    let text: String = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| buf.cell((x, y)).unwrap().symbol().to_string())
        .collect();
    (text, cur)
}

// ---- tab bar visibility ----------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn single_tab_hides_tab_bar_and_renders_doc() {
    let (mut app, _d, _v) = app_with_files(&[("a.md", "hello world\n")]).await;
    open_doc(&mut app, "hello world\n");
    let (text, _c) = render(&mut app, 60, 12);
    assert!(text.contains("hello world"), "got: {text:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn multiple_tabs_show_the_tab_bar() {
    let (mut app, _d, _v) = app_with_files(&[("a.md", "alpha\n")]).await;
    open_doc(&mut app, "alpha content\n");
    // Add a second tab → show_tabs branch.
    let pane2 = Pane::new(
        Document::from_markdown("beta content\n").unwrap(),
        PathBuf::from("b.md"),
    );
    app.tabs.tabs.push(TabState::new(pane2));
    let (text, _c) = render(&mut app, 70, 14);
    assert!(app.tabs.len() > 1);
    assert!(text.contains("alpha content"), "got: {text:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn empty_state_when_no_tabs() {
    let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
    app.tabs.tabs.clear();
    let (text, _c) = render(&mut app, 60, 10);
    assert!(
        text.contains("no markdown files yet"),
        "expected empty-state hint: {text:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn single_empty_leaf_renders_empty_state_inline() {
    let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
    app.tabs.tabs = vec![TabState::new(Pane::empty())];
    app.tabs.active = 0;
    let (text, _c) = render(&mut app, 60, 10);
    assert!(text.contains("no markdown files yet"), "got: {text:?}");
}

// ---- tree sidebar ----------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn tree_visible_splits_body_and_paints_sidebar() {
    let (mut app, _d, _v) = app_with_files(&[("a.md", "doc body\n")]).await;
    open_doc(&mut app, "doc body\n");
    app.tree.visible = true;
    let (text, _c) = render(&mut app, 80, 16);
    // Editor content still shows next to the sidebar.
    assert!(text.contains("doc body"), "got: {text:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn tree_hidden_uses_full_width_editor() {
    let (mut app, _d, _v) = app_with_files(&[("a.md", "wide editor\n")]).await;
    open_doc(&mut app, "wide editor\n");
    app.tree.visible = false;
    let (text, _c) = render(&mut app, 80, 12);
    assert!(text.contains("wide editor"), "got: {text:?}");
}

// ---- git side panel --------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn git_panel_visible_paints_right_column() {
    // Vault isn't a git repo here; the renderer surfaces the
    // "no repo" / fatal message instead of crashing, which is the
    // header contract.
    let (mut app, _d, _v) = app_with_files(&[("a.md", "doc body\n")]).await;
    open_doc(&mut app, "doc body\n");
    app.git_panel.visible = true;
    app.git_panel.status_error = Some("fatal: not a git repository".to_string());
    let (text, _c) = render(&mut app, 100, 18);
    assert!(text.contains("doc body"), "editor preserved: {text:?}");
    assert!(text.contains("Git"), "panel title painted: {text:?}");
    assert!(text.contains("no repo"), "header label painted: {text:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn git_panel_hidden_does_not_paint_column() {
    let (mut app, _d, _v) = app_with_files(&[("a.md", "doc body\n")]).await;
    open_doc(&mut app, "doc body\n");
    app.git_panel.visible = false;
    let (text, _c) = render(&mut app, 100, 14);
    assert!(
        !text.contains("Git —"),
        "no panel title when hidden: {text:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn git_panel_and_tree_share_body_with_editor_in_middle() {
    let (mut app, _d, _v) = app_with_files(&[("a.md", "middle slot\n")]).await;
    open_doc(&mut app, "middle slot\n");
    app.tree.visible = true;
    app.git_panel.visible = true;
    let (text, _c) = render(&mut app, 110, 16);
    // All three columns coexist: tree title, editor content, panel title.
    assert!(text.contains("httui"), "tree painted: {text:?}");
    assert!(text.contains("middle slot"), "editor painted: {text:?}");
    assert!(text.contains("Git"), "git panel painted: {text:?}");
}

// ---- mode-specific cursor placement ----------------------------

#[tokio::test(flavor = "multi_thread")]
async fn command_line_mode_places_cursor_in_status_row() {
    let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
    open_doc(&mut app, "x\n");
    app.vim.mode = Mode::CommandLine;
    app.modal = Some(crate::modal::Modal::Prompt(
        crate::modal::PromptKind::Cmdline,
        crate::vim::lineedit::LineEdit::from_str("w"),
    ));
    let (_t, cur) = render(&mut app, 60, 10);
    assert!(cur.is_some(), "command-line cursor should be set");
}

#[tokio::test(flavor = "multi_thread")]
async fn search_mode_places_cursor_and_live_highlights() {
    let (mut app, _d, _v) = app_with_files(&[("a.md", "find me here\n")]).await;
    open_doc(&mut app, "find me here\n");
    app.vim.mode = Mode::Search;
    app.modal = Some(crate::modal::Modal::Prompt(
        crate::modal::PromptKind::Search { forward: true },
        crate::vim::lineedit::LineEdit::from_str("me"),
    ));
    let (_t, cur) = render(&mut app, 60, 10);
    assert!(cur.is_some(), "search cursor should be set");
}

#[tokio::test(flavor = "multi_thread")]
async fn quickopen_mode_renders_and_sets_cursor() {
    let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
    open_doc(&mut app, "x\n");
    app.vim.mode = Mode::QuickOpen;
    let (_t, cur) = render(&mut app, 60, 12);
    assert!(cur.is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn content_search_mode_renders_when_state_present() {
    let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
    open_doc(&mut app, "x\n");
    app.vim.mode = Mode::ContentSearch;
    app.modal = Some(crate::modal::Modal::ContentSearch(ContentSearchState::new()));
    let (_t, cur) = render(&mut app, 60, 12);
    assert!(cur.is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn tree_prompt_mode_places_cursor_in_status_row() {
    let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
    open_doc(&mut app, "x\n");
    app.vim.mode = Mode::TreePrompt;
    app.tree.prompt = Some(crate::tree::TreePrompt::new(
        crate::tree::TreePromptKind::Create { dir: String::new() },
        "new.md".into(),
    ));
    let (_t, cur) = render(&mut app, 60, 10);
    assert!(cur.is_some(), "tree-prompt cursor should be set");
}

#[tokio::test(flavor = "multi_thread")]
async fn suppress_cursor_when_modal_owns_input() {
    let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
    open_doc(&mut app, "x\n");
    app.modal = Some(crate::modal::Modal::Help);
    let (text, _c) = render(&mut app, 60, 12);
    // Help modal painted on top.
    assert!(!text.trim().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn completion_popup_does_not_suppress_editor_cursor() {
    let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
    open_doc(&mut app, "x\n");
    app.modal = Some(crate::modal::Modal::CompletionPopup(CompletionPopupState {
        segment_idx: 0,
        items: vec![crate::sql_completion::CompletionItem {
            label: "TOKEN".into(),
            kind: crate::sql_completion::CompletionKind::Reference,
            detail: Some("env".into()),
        }],
        selected: 0,
        anchor_line: 0,
        anchor_offset: 0,
        prefix: String::new(),
    }));
    let (_text, cur) = render(&mut app, 60, 12);
    assert!(
        cur.is_some(),
        "CompletionPopup is passive — editor cursor must remain",
    );
}

// ---- visual overlay paths --------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn visual_mode_overlay_path_is_exercised() {
    let (mut app, _d, _v) = app_with_files(&[("a.md", "select this line\n")]).await;
    open_doc(&mut app, "select this line\n");
    app.config.editor.mode = EditorMode::Vim;
    app.vim.mode = Mode::Visual;
    app.vim.visual_anchor = Some(Cursor::InProse {
        segment_idx: 0,
        offset: 0,
    });
    if let Some(d) = app.tabs.active_document_mut() {
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 6,
        });
    }
    let (text, _c) = render(&mut app, 60, 10);
    assert!(text.contains("select this line"), "got: {text:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn visual_line_mode_overlay_path_is_exercised() {
    let (mut app, _d, _v) = app_with_files(&[("a.md", "line A\nline B\n")]).await;
    open_doc(&mut app, "line A\nline B\n");
    app.vim.mode = Mode::VisualLine;
    app.vim.visual_anchor = Some(Cursor::InProse {
        segment_idx: 0,
        offset: 0,
    });
    let (text, _c) = render(&mut app, 60, 10);
    assert!(text.contains("line A"), "got: {text:?}");
}

// ---- modal / popup overlay stack -------------------------------

fn sub_doc() -> Document {
    Document::from_markdown("status 200\nheader: x\n\nbody\n").unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn db_row_detail_modal_paints() {
    let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
    open_doc(&mut app, "x\n");
    app.modal = Some(crate::modal::Modal::DbRowDetail(DbRowDetailState {
        segment_idx: 0,
        row: 0,
        title: " row 1 ".into(),
        doc: sub_doc(),
        viewport_height: 10,
        viewport_top: 0,
    }));
    let (text, _c) = render(&mut app, 70, 16);
    assert!(text.contains("body"), "got: {text:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn http_response_detail_modal_paints_with_visual() {
    let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
    open_doc(&mut app, "x\n");
    app.vim.mode = Mode::Visual;
    app.vim.visual_anchor = Some(Cursor::InProse {
        segment_idx: 0,
        offset: 0,
    });
    app.modal = Some(crate::modal::Modal::HttpResponseDetail(
        HttpResponseDetailState {
            segment_idx: 0,
            title: " 200 OK ".into(),
            doc: sub_doc(),
            viewport_height: 10,
            viewport_top: 0,
        },
    ));
    let (text, _c) = render(&mut app, 70, 16);
    assert!(
        text.contains("status 200") || text.contains("body"),
        "got: {text:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn connection_picker_popup_paints_anchored() {
    let src = "```db-postgres alias=q connection=c\nSELECT 1\n```\n";
    let (mut app, _d, _v) = app_with_files(&[("a.md", src)]).await;
    open_doc(&mut app, src);
    let seg = app
        .tabs
        .active_document_mut()
        .unwrap()
        .segments()
        .iter()
        .position(|s| matches!(s, crate::buffer::Segment::Block(_)))
        .unwrap();
    app.modal = Some(crate::modal::Modal::ConnectionPicker(
        ConnectionPickerState {
            segment_idx: seg,
            connections: vec![crate::app::ConnectionEntry {
                id: "c1".into(),
                name: "Local PG".into(),
                kind: "postgres".into(),
            }],
            selected: 0,
        },
    ));
    let (text, _c) = render(&mut app, 70, 16);
    assert!(text.contains("Local PG"), "got: {text:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn fence_edit_popup_paints() {
    let src = "```db-postgres alias=q connection=c\nSELECT 1\n```\n";
    let (mut app, _d, _v) = app_with_files(&[("a.md", src)]).await;
    open_doc(&mut app, src);
    app.modal = Some(crate::modal::Modal::Prompt(
        crate::modal::PromptKind::FenceEditAlias { segment_idx: 0 },
        crate::vim::lineedit::LineEdit::from_str("req1"),
    ));
    let (text, _c) = render(&mut app, 70, 14);
    assert!(
        text.contains("req1") || text.contains("alias"),
        "got: {text:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn completion_popup_paints() {
    let src = "```db-postgres alias=q connection=c\nSELECT 1\n```\n";
    let (mut app, _d, _v) = app_with_files(&[("a.md", src)]).await;
    open_doc(&mut app, src);
    app.modal = Some(crate::modal::Modal::CompletionPopup(CompletionPopupState {
        segment_idx: 0,
        items: vec![crate::sql_completion::CompletionItem {
            label: "SELECT".into(),
            kind: crate::sql_completion::CompletionKind::Keyword,
            detail: None,
        }],
        selected: 0,
        anchor_line: 0,
        anchor_offset: 0,
        prefix: "SEL".into(),
    }));
    let (text, _c) = render(&mut app, 70, 16);
    assert!(text.contains("SELECT"), "got: {text:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn db_confirm_run_modal_paints() {
    let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
    open_doc(&mut app, "x\n");
    app.modal = Some(crate::modal::Modal::DbConfirmRun(DbConfirmRunState {
        segment_idx: 0,
        reason: "UPDATE without WHERE".into(),
    }));
    let (text, _c) = render(&mut app, 70, 14);
    assert!(
        text.contains("WHERE") || text.contains("UPDATE"),
        "got: {text:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn db_export_picker_modal_paints() {
    let src = "```db-postgres alias=q connection=c\nSELECT 1\n```\n";
    let (mut app, _d, _v) = app_with_files(&[("a.md", src)]).await;
    open_doc(&mut app, src);
    app.modal = Some(crate::modal::Modal::DbExportPicker(
        DbExportPickerState::new(0, BlockExportFormat::DB_FORMATS),
    ));
    let (text, _c) = render(&mut app, 70, 16);
    assert!(
        text.contains("CSV") || text.contains("JSON"),
        "got: {text:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn db_settings_modal_paints() {
    let src = "```db-postgres alias=q connection=c\nSELECT 1\n```\n";
    let (mut app, _d, _v) = app_with_files(&[("a.md", src)]).await;
    open_doc(&mut app, src);
    app.modal = Some(crate::modal::Modal::DbSettings(DbSettingsState {
        segment_idx: 0,
        fields: vec![SettingsField {
            label: "Limit",
            key: "limit",
            input: crate::vim::lineedit::LineEdit::from_str("100"),
        }],
        focus: 0,
    }));
    let (text, _c) = render(&mut app, 70, 16);
    assert!(
        text.contains("Limit") || text.contains("100"),
        "got: {text:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn block_history_modal_paints() {
    let src = "```http alias=h\nGET https://x.com\n```\n";
    let (mut app, _d, _v) = app_with_files(&[("a.md", src)]).await;
    open_doc(&mut app, src);
    app.modal = Some(crate::modal::Modal::BlockHistory(BlockHistoryState {
        segment_idx: 0,
        title: "GET h".into(),
        entries: vec![],
        selected: 0,
    }));
    let (text, _c) = render(&mut app, 70, 16);
    assert!(
        text.contains("GET h") || !text.trim().is_empty(),
        "got: {text:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn environment_picker_modal_paints() {
    let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
    open_doc(&mut app, "x\n");
    app.modal = Some(crate::modal::Modal::EnvironmentPicker(
        EnvironmentPickerState {
            entries: vec![crate::app::EnvironmentEntry {
                id: "e1".into(),
                name: "staging".into(),
            }],
            selected: 0,
            active_id: None,
        },
    ));
    let (text, _c) = render(&mut app, 70, 14);
    assert!(text.contains("staging"), "got: {text:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn help_modal_paints() {
    let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
    open_doc(&mut app, "x\n");
    app.modal = Some(crate::modal::Modal::Help);
    let (text, _c) = render(&mut app, 80, 24);
    assert!(!text.trim().is_empty(), "help modal should paint");
}

#[tokio::test(flavor = "multi_thread")]
async fn block_template_picker_modal_paints() {
    let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
    open_doc(&mut app, "x\n");
    app.modal = Some(crate::modal::Modal::BlockTemplatePicker(
        BlockTemplatePickerState::new(),
    ));
    let (text, _c) = render(&mut app, 70, 16);
    assert!(
        text.contains("HTTP") || !text.trim().is_empty(),
        "got: {text:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn tab_picker_modal_paints() {
    let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
    open_doc(&mut app, "x\n");
    app.modal = Some(crate::modal::Modal::TabPicker(TabPickerState {
        entries: vec![crate::app::TabPickerEntry {
            idx: 0,
            label: "a.md".into(),
            dirty: false,
        }],
        selected: 0,
    }));
    let (text, _c) = render(&mut app, 70, 14);
    assert!(text.contains("a.md"), "got: {text:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn noh_search_no_highlight_path() {
    // search_highlight false + not in Search mode → search_pattern
    // resolves to None (the `:noh` branch).
    let (mut app, _d, _v) = app_with_files(&[("a.md", "plain text\n")]).await;
    open_doc(&mut app, "plain text\n");
    app.vim.mode = Mode::Normal;
    app.vim.search_highlight = false;
    let (text, _c) = render(&mut app, 60, 8);
    assert!(text.contains("plain text"), "got: {text:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn persisted_search_highlight_uses_last_search() {
    let (mut app, _d, _v) = app_with_files(&[("a.md", "find token here\n")]).await;
    open_doc(&mut app, "find token here\n");
    app.vim.mode = Mode::Normal;
    app.vim.search_highlight = true;
    app.vim.last_search = Some("token".into());
    let (text, _c) = render(&mut app, 60, 8);
    assert!(text.contains("find token here"), "got: {text:?}");
}

// ---- Standard-profile selection overlay (fase 3 P-W) -----------

/// Same as `render` but returns the raw cell buffer so a test can
/// assert per-cell `bg`. The selection overlay only mutates the
/// background, so the bg is the load-bearing signal.
fn render_buf(app: &mut App, w: u16, h: u16) -> ratatui::buffer::Buffer {
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            super::render(f, app);
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

fn sel_bg() -> ratatui::style::Color {
    crate::ui::palette::selection_bg()
}

#[tokio::test(flavor = "multi_thread")]
async fn standard_mode_selection_paints_highlight_bg() {
    // Cenário 1 passo 5 (pleno): Standard profile, a non-empty
    // Shift-selection (anchor + moved caret) → cells in the range
    // carry the selection bg. Proves the highlight is *visible*.
    let (mut app, _d, _v) = app_with_files(&[("a.md", "select this line\n")]).await;
    open_doc(&mut app, "select this line\n");
    app.config.editor.mode = EditorMode::Standard;
    app.vim.mode = Mode::Normal; // Standard has no Mode::Visual
    app.vim.visual_anchor = None;
    app.standard.anchor = Some(Cursor::InProse {
        segment_idx: 0,
        offset: 0,
    });
    if let Some(d) = app.tabs.active_document_mut() {
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 6,
        });
    }
    let buf = render_buf(&mut app, 60, 10);
    let painted = (0..10)
        .flat_map(|y| (0..60).map(move |x| (x, y)))
        .filter(|&(x, y)| buf.cell((x, y)).unwrap().bg == sel_bg())
        .count();
    assert!(
        painted >= 6,
        "Standard selection must paint the highlight bg over the \
             selected run; painted {painted} cells with sel_bg()"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn vim_profile_ignores_standard_anchor_no_highlight() {
    // Guarda Cenário 2: Vim profile, no vim visual anchor, but a
    // stray `standard.anchor` set → the Standard arm resolves to
    // `None` (helper returns None for Vim), so NO cell gets the
    // selection bg. The Standard path can never leak into vim.
    let (mut app, _d, _v) = app_with_files(&[("a.md", "select this line\n")]).await;
    open_doc(&mut app, "select this line\n");
    app.config.editor.mode = EditorMode::Vim;
    app.vim.mode = Mode::Normal;
    app.vim.visual_anchor = None;
    app.standard.anchor = Some(Cursor::InProse {
        segment_idx: 0,
        offset: 0,
    });
    if let Some(d) = app.tabs.active_document_mut() {
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 6,
        });
    }
    let buf = render_buf(&mut app, 60, 10);
    let painted = (0..10)
        .flat_map(|y| (0..60).map(move |x| (x, y)))
        .filter(|&(x, y)| buf.cell((x, y)).unwrap().bg == sel_bg())
        .count();
    assert_eq!(
        painted, 0,
        "Vim profile must never paint a Standard-anchor selection \
             (Cenário 2 byte-identical); painted {painted}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn render_in_search_mode_with_non_search_prompt_does_not_blow_up() {
    // Exercise the `_` fallback of the live-search prompt match in
    // render(): mode is Search but the active modal isn't a Search
    // prompt (e.g. Cmdline). The branch should produce live_search_buf
    // = None and render cleanly.
    let (mut app, _d, _v) = app_with_files(&[]).await;
    open_doc(&mut app, "alpha\n");
    app.vim.mode = Mode::Search;
    app.modal = Some(crate::modal::Modal::Prompt(
        crate::modal::PromptKind::Cmdline,
        crate::vim::lineedit::LineEdit::new(),
    ));
    let (_frame, _cursor) = render(&mut app, 60, 6);
}

#[tokio::test(flavor = "multi_thread")]
async fn render_in_search_mode_with_empty_search_prompt_does_not_blow_up() {
    // Empty search buffer hits the guard `!le.is_empty()` → falls
    // through to the `_` arm, returning None.
    let (mut app, _d, _v) = app_with_files(&[]).await;
    open_doc(&mut app, "alpha\n");
    app.vim.mode = Mode::Search;
    app.modal = Some(crate::modal::Modal::Prompt(
        crate::modal::PromptKind::Search { forward: true },
        crate::vim::lineedit::LineEdit::new(),
    ));
    let (_frame, _cursor) = render(&mut app, 60, 6);
}

#[tokio::test(flavor = "multi_thread")]
async fn render_with_active_search_buffer_paints_live_match() {
    let (mut app, _d, _v) = app_with_files(&[]).await;
    open_doc(&mut app, "alpha beta gamma\n");
    app.vim.mode = Mode::Search;
    app.modal = Some(crate::modal::Modal::Prompt(
        crate::modal::PromptKind::Search { forward: true },
        crate::vim::lineedit::LineEdit::from_str("beta"),
    ));
    let (_frame, _cursor) = render(&mut app, 80, 10);
}

#[tokio::test(flavor = "multi_thread")]
async fn render_paints_env_var_delete_confirm_modals() {
    let (mut app, _d, _v) = app_with_files(&[]).await;
    open_doc(&mut app, "x\n");
    app.modal = Some(crate::modal::Modal::EnvDeleteConfirm(
        crate::app::EnvDeleteConfirmState { name: "dev".into() },
    ));
    let _ = render(&mut app, 80, 20);
    app.modal = Some(crate::modal::Modal::VarDeleteConfirm(
        crate::app::VarDeleteConfirmState {
            env_name: "dev".into(),
            key: "KEY".into(),
        },
    ));
    let _ = render(&mut app, 80, 20);
}

#[tokio::test(flavor = "multi_thread")]
async fn render_paints_settings_page_modal() {
    let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
    open_doc(&mut app, "x\n");
    app.modal = Some(crate::modal::Modal::Settings(
        crate::app::SettingsPageState::new(std::path::PathBuf::from("/tmp/cfg.toml")),
    ));
    let (text, _) = render(&mut app, 120, 24);
    assert!(text.contains("Settings"));
    assert!(text.contains("Keymaps"));
}

#[tokio::test(flavor = "multi_thread")]
async fn render_paints_env_form_and_var_form() {
    let (mut app, _d, _v) = app_with_files(&[]).await;
    open_doc(&mut app, "x\n");
    app.modal = Some(crate::modal::Modal::EnvForm(crate::app::EnvFormState {
        name: crate::vim::lineedit::LineEdit::from_str("dev"),
        editing: None,
        error: None,
    }));
    let _ = render(&mut app, 80, 20);
    app.modal = Some(crate::modal::Modal::VarForm(crate::app::VarFormState {
        env_name: "dev".into(),
        key: crate::vim::lineedit::LineEdit::from_str("K"),
        value: crate::vim::lineedit::LineEdit::from_str("V"),
        is_secret: false,
        focus: crate::app::VarFormFocus::Key,
        editing: None,
        error: None,
    }));
    let _ = render(&mut app, 80, 20);
}

// ---- BLOCKS view render coverage -------------------------------

async fn blocks_app() -> (App, TempDir, TempDir) {
    let (mut app, data, vault) = app_with_files(&[
        (
            "api.md",
            "# api\n\n```http alias=req1\nGET https://x.com\nAuthorization: Bearer {{TOKEN}}\n```\n",
        ),
        ("db.md", "# db\n\n```db-postgres alias=q1\nSELECT 1\n```\n"),
    ])
    .await;
    crate::input::apply::blocks_view::apply_blocks_view(
        &mut app,
        crate::input::action::Action::ToggleAppView,
    );
    (app, data, vault)
}

fn block_ref_of(app: &App, want_http: bool) -> crate::app::BlockRef {
    let ws = app.blocks_workspace.as_ref().expect("workspace built");
    for (fi, f) in ws.index.files.iter().enumerate() {
        for (bi, b) in f.blocks.iter().enumerate() {
            if (b.block_type == "http") == want_http {
                return crate::app::BlockRef {
                    file_idx: fi,
                    block_idx: bi,
                };
            }
        }
    }
    panic!("no matching block in index");
}

fn select_ref(app: &mut App, sel: crate::app::BlockRef) {
    if let Some(ws) = app.blocks_workspace.as_mut() {
        ws.selected = Some(sel);
    }
    if let Some(p) = app.active_pane_mut() {
        p.block_selected = Some(sel);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn blocks_view_renders_http_block() {
    let (mut app, _d, _v) = blocks_app().await;
    let sel = block_ref_of(&app, true);
    select_ref(&mut app, sel);
    let (text, _) = render(&mut app, 120, 40);
    assert!(text.contains("REQUEST"), "expected REQUEST card: {text:?}");
    assert!(text.contains("RESPONSE"));
}

#[tokio::test(flavor = "multi_thread")]
async fn blocks_view_renders_db_block() {
    let (mut app, _d, _v) = blocks_app().await;
    let sel = block_ref_of(&app, false);
    select_ref(&mut app, sel);
    let (text, _) = render(&mut app, 120, 40);
    assert!(text.contains("QUERY"), "expected QUERY card: {text:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn blocks_view_empty_selection_paints_placeholder() {
    let (mut app, _d, _v) = blocks_app().await;
    let (text, _) = render(&mut app, 120, 40);
    assert!(text.contains("Select a block"), "expected placeholder: {text:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn blocks_view_renders_vertical_split() {
    let (mut app, _d, _v) = blocks_app().await;
    let http = block_ref_of(&app, true);
    let db = block_ref_of(&app, false);
    select_ref(&mut app, http);
    let active = app.tabs.active;
    let tab = app.tabs.tabs.get_mut(active).unwrap();
    let old = std::mem::replace(&mut tab.root, crate::pane::PaneNode::Leaf(crate::pane::Pane::empty()));
    let mut second = crate::pane::Pane::empty();
    second.block_selected = Some(db);
    tab.root = crate::pane::PaneNode::Split {
        direction: crate::pane::SplitDir::Vertical,
        ratio: 0.5,
        first: Box::new(old),
        second: Box::new(crate::pane::PaneNode::Leaf(second)),
    };
    // Focus must point at a leaf, not the new split root.
    tab.focused = vec![0];
    let (text, _) = render(&mut app, 160, 40);
    assert!(text.contains("REQUEST"));
    assert!(text.contains("QUERY"));
}

#[tokio::test(flavor = "multi_thread")]
async fn blocks_view_renders_edit_buffer() {
    let (mut app, _d, _v) = blocks_app().await;
    let sel = block_ref_of(&app, true);
    select_ref(&mut app, sel);
    crate::input::apply::blocks_view::apply_blocks_view(
        &mut app,
        crate::input::action::Action::BlocksRegionEnterEdit,
    );
    let (text, _) = render(&mut app, 120, 40);
    assert!(text.contains("EDIT"), "edit chip expected: {text:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn blocks_view_renders_http_response_with_cached_result() {
    let (mut app, _d, v) = blocks_app().await;
    let sel = block_ref_of(&app, true);
    select_ref(&mut app, sel);
    let text = httui_core::fs::read_note(&v.path().to_string_lossy(), "api.md").unwrap();
    let mut doc = Document::from_markdown(&text).unwrap();
    let idx = doc
        .segments()
        .iter()
        .position(|s| matches!(s, crate::buffer::Segment::Block(b) if b.block_type == "http"))
        .unwrap();
    if let Some(b) = doc.block_at_mut(idx) {
        b.cached_result = Some(serde_json::json!({
            "status": 200, "status_text": "OK",
            "headers": [{"key": "content-type", "value": "application/json"}],
            "body": "{\"ok\":true}", "elapsed_ms": 12, "size_bytes": 11
        }));
    }
    if let Some(p) = app.active_pane_mut() {
        p.document = Some(doc);
        p.document_path = Some(v.path().join("api.md"));
        p.block_region = 3;
    }
    let (text, _) = render(&mut app, 120, 40);
    assert!(text.contains("200"), "status badge expected: {text:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn blocks_unsaved_prompt_renders_over_pane() {
    let (mut app, _d, _v) = blocks_app().await;
    let sel = block_ref_of(&app, true);
    select_ref(&mut app, sel);
    app.modal = Some(crate::modal::Modal::BlocksUnsavedPrompt(
        crate::app::BlocksUnsavedPromptState {
            dirty: vec![std::path::PathBuf::from("api.md")],
            focus: crate::app::BlocksUnsavedPromptFocus::default(),
        },
    ));
    let (text, _) = render(&mut app, 120, 40);
    assert!(!text.trim().is_empty());
}

fn enter_edit_region(app: &mut App, region: usize) {
    if let Some(p) = app.active_pane_mut() {
        p.block_region = region;
    }
    crate::input::apply::blocks_view::apply_blocks_view(
        app,
        crate::input::action::Action::BlocksRegionEnterEdit,
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn blocks_view_renders_http_headers_edit() {
    let (mut app, _d, _v) = blocks_app().await;
    let sel = block_ref_of(&app, true);
    select_ref(&mut app, sel);
    enter_edit_region(&mut app, 1); // Headers value
    let (text, cur) = render(&mut app, 120, 40);
    assert!(text.contains("EDIT"));
    assert!(cur.is_some(), "caret placed in edited header cell");
}

#[tokio::test(flavor = "multi_thread")]
async fn blocks_view_renders_http_body_edit() {
    let (mut app, _d, _v) = blocks_app().await;
    let sel = block_ref_of(&app, true);
    select_ref(&mut app, sel);
    enter_edit_region(&mut app, 2); // Body
    let (text, _) = render(&mut app, 120, 40);
    assert!(text.contains("EDIT"));
}

#[tokio::test(flavor = "multi_thread")]
async fn blocks_view_renders_db_query_edit() {
    let (mut app, _d, _v) = blocks_app().await;
    let sel = block_ref_of(&app, false);
    select_ref(&mut app, sel);
    enter_edit_region(&mut app, 1); // Query
    let (text, _) = render(&mut app, 120, 40);
    assert!(text.contains("SELECT") || text.contains("EDIT"));
}

#[tokio::test(flavor = "multi_thread")]
async fn blocks_view_renders_db_result_table() {
    let (mut app, _d, v) = blocks_app().await;
    let sel = block_ref_of(&app, false);
    select_ref(&mut app, sel);
    let text = httui_core::fs::read_note(&v.path().to_string_lossy(), "db.md").unwrap();
    let mut doc = Document::from_markdown(&text).unwrap();
    let idx = doc
        .segments()
        .iter()
        .position(|s| matches!(s, crate::buffer::Segment::Block(b) if b.block_type.starts_with("db")))
        .unwrap();
    if let Some(b) = doc.block_at_mut(idx) {
        b.cached_result = Some(serde_json::json!({
            "results": [{
                "kind": "select",
                "columns": ["id", "name"],
                "rows": [{"id": 1, "name": "a"}, {"id": 2, "name": "b"}]
            }]
        }));
    }
    if let Some(p) = app.active_pane_mut() {
        p.document = Some(doc);
        p.document_path = Some(v.path().join("db.md"));
        p.block_region = 2; // Result
    }
    let (text, _) = render(&mut app, 120, 40);
    // Rows rendered → not the empty placeholder, so the table-build
    // path in `render_db_result_region` executed.
    assert!(!text.contains("no result"), "result table should render rows: {text:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn blocks_view_renders_pane_picker_overlay() {
    let (mut app, _d, _v) = blocks_app().await;
    let sel = block_ref_of(&app, true);
    select_ref(&mut app, sel);
    if let Some(ws) = app.blocks_workspace.as_mut() {
        ws.pane_picker = Some(sel);
    }
    let (text, _) = render(&mut app, 120, 40);
    assert!(text.contains("[ a ]"), "picker label expected: {text:?}");
}
