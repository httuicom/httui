use ratatui::style::Color;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Normal,
    Insert,
    CommandLine,
    /// `/` (forward) or `?` (backward) prompt. Direction is stored on
    /// [`super::state::VimState::search_forward`].
    Search,
    /// `Ctrl+P` quick-open modal. Buffer + filtered results live on
    /// [`super::state::VimState::quickopen`].
    QuickOpen,
    /// File-tree sidebar focused. Editor stays painted but isn't
    /// receiving keys. Toggle with `Ctrl+E`; switch focus with Tab.
    Tree,
    /// Tree-driven prompt for `a`/`r`/`d` (create / rename / delete).
    /// The input UI lives in the status bar but the action it runs is
    /// a feature, not an ex command.
    TreePrompt,
    /// `v` — character-wise visual selection. Anchor lives on
    /// [`super::state::VimState::visual_anchor`]; the moving end is
    /// the document cursor. Motions extend, `d`/`c`/`y`/`x` operate.
    Visual,
    /// `V` — line-wise visual selection. Selects entire lines from
    /// the anchor's line to the cursor's line.
    VisualLine,
    /// `<CR>` on a DB result row opens a centered modal with the
    /// row's columns spelled out in full (JSON pretty-printed). All
    /// keys flow into the modal until it's dismissed; the editor
    /// underneath is frozen but kept painted.
    DbRowDetail,
    /// `<CR>` on an HTTP response panel opens a centered modal with
    /// the full response body + status/headers/timing summary. Same
    /// rendering trick as `DbRowDetail`: a sub-`Document` on the
    /// state struct receives every motion via `parse_normal`, so the
    /// editor's full vim vocabulary navigates the modal.
    HttpResponseDetail,
    /// Inline fence-edit prompt for one of the block's metadata
    /// fields (alias / limit / timeout). State lives on
    /// `App.fence_edit`; the prompt renders in the status bar like
    /// `TreePrompt` so the editor under it stays visible.
    FenceEdit,
    /// `gs` on a DB block — open the settings modal with multiple
    /// fields (limit + timeout). Tab cycles fields, Enter saves all,
    /// Esc cancels. State lives on `App.db_settings`. Picked over
    /// chord-per-field (`gl`/`gw`) per the
    /// `project_tui_block_settings_modal.md` user-memory.
    DbSettings,
    /// `<C-f>` — open the content-search modal. Per-keystroke FTS5
    /// query over `httui-core::search::search_index`. Up/Down (or
    /// Ctrl-n/p) navigate; Enter opens the picked file in a new tab.
    ContentSearch,
    Modal,
}

impl Mode {
    pub fn label(&self) -> &'static str {
        match self {
            Mode::Normal => "NOR",
            Mode::Insert => "INS",
            Mode::CommandLine => "CMD",
            Mode::Search => "SEA",
            Mode::QuickOpen => "OPEN",
            Mode::Tree => "TREE",
            Mode::TreePrompt => "TREE",
            Mode::Visual => "VIS",
            Mode::VisualLine => "V-L",
            Mode::DbRowDetail => "ROW",
            Mode::HttpResponseDetail => "RESP",
            Mode::FenceEdit => "EDIT",
            Mode::DbSettings => "SET",
            Mode::ContentSearch => "FIND",
            Mode::Modal => "MOD",
        }
    }

    pub fn bg(&self) -> Color {
        match self {
            Mode::Normal => Color::LightCyan,
            Mode::Insert => Color::LightYellow,
            Mode::CommandLine => Color::LightMagenta,
            Mode::Search => Color::LightGreen,
            Mode::QuickOpen => Color::LightBlue,
            Mode::Tree | Mode::TreePrompt => Color::Yellow,
            Mode::Visual | Mode::VisualLine => Color::LightRed,
            Mode::DbRowDetail => Color::LightBlue,
            Mode::HttpResponseDetail => Color::LightBlue,
            Mode::FenceEdit => Color::LightYellow,
            Mode::DbSettings => Color::LightYellow,
            Mode::ContentSearch => Color::LightGreen,
            Mode::Modal => Color::LightBlue,
        }
    }
}
