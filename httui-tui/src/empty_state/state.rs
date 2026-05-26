//! Pure state machine for the empty-state bootstrap. Holds the four
//! screens (Cards / Create / Clone / Open) and the cursor inside Cards.
//! No terminal I/O, no async — fully testable.

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{
    VaultCloneFormFocus, VaultCloneFormState, VaultCreateFormFocus, VaultCreateFormState,
    VaultOpenEntryKind, VaultOpenPickerState,
};
use crate::vault::helpers;
use crate::vim::lineedit::LineEdit;

/// Active screen. The bootstrap always starts on `Cards`; opening any
/// other screen comes from confirming a card. Closing a sub-screen
/// (Esc) returns to `Cards`.
#[derive(Debug)]
pub enum Screen {
    Cards,
    Create(VaultCreateFormState),
    Clone(VaultCloneFormState),
    Open(VaultOpenPickerState),
}

/// Highlighted card. `Open` is first because it's the most common
/// path (existing notes dir on disk).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardChoice {
    Open,
    Clone,
    Create,
}

impl CardChoice {
    pub const ALL: [CardChoice; 3] = [CardChoice::Open, CardChoice::Clone, CardChoice::Create];

    fn next(self) -> Self {
        match self {
            CardChoice::Open => CardChoice::Clone,
            CardChoice::Clone => CardChoice::Create,
            CardChoice::Create => CardChoice::Open,
        }
    }

    fn prev(self) -> Self {
        match self {
            CardChoice::Open => CardChoice::Create,
            CardChoice::Clone => CardChoice::Open,
            CardChoice::Create => CardChoice::Clone,
        }
    }
}

#[derive(Debug)]
pub struct BootstrapState {
    pub screen: Screen,
    pub selected_card: CardChoice,
}

impl Default for BootstrapState {
    fn default() -> Self {
        Self {
            screen: Screen::Cards,
            selected_card: CardChoice::Open,
        }
    }
}

/// Outcome of a key event. The async caller checks `Activated` to
/// `set_active_vault` + return, or `Quit` to abort the binary with
/// a clean error.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Continue,
    Quit,
    Activated(PathBuf),
}

impl BootstrapState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        // Ctrl-C is a universal cancel from any screen.
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Outcome::Quit;
        }
        match &mut self.screen {
            Screen::Cards => handle_cards(&mut self.screen, &mut self.selected_card, key),
            Screen::Create(_) => handle_create(&mut self.screen, key),
            Screen::Clone(_) => handle_clone(&mut self.screen, key),
            Screen::Open(_) => handle_open(&mut self.screen, key),
        }
    }
}

fn handle_cards(screen: &mut Screen, selected: &mut CardChoice, key: KeyEvent) -> Outcome {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => Outcome::Quit,
        KeyCode::Left | KeyCode::Char('h') | KeyCode::BackTab => {
            *selected = selected.prev();
            Outcome::Continue
        }
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => {
            *selected = selected.next();
            Outcome::Continue
        }
        KeyCode::Up | KeyCode::Char('k') => {
            *selected = selected.prev();
            Outcome::Continue
        }
        KeyCode::Down | KeyCode::Char('j') => {
            *selected = selected.next();
            Outcome::Continue
        }
        KeyCode::Char('o') => {
            *selected = CardChoice::Open;
            open_screen_for(screen, CardChoice::Open);
            Outcome::Continue
        }
        KeyCode::Char('n') => {
            *selected = CardChoice::Create;
            open_screen_for(screen, CardChoice::Create);
            Outcome::Continue
        }
        KeyCode::Char('g') => {
            *selected = CardChoice::Clone;
            open_screen_for(screen, CardChoice::Clone);
            Outcome::Continue
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            open_screen_for(screen, *selected);
            Outcome::Continue
        }
        _ => Outcome::Continue,
    }
}

fn open_screen_for(screen: &mut Screen, card: CardChoice) {
    match card {
        CardChoice::Create => {
            let default_parent = std::env::var("HOME").ok().unwrap_or_else(|| ".".into());
            *screen = Screen::Create(VaultCreateFormState {
                parent: LineEdit::from_str(default_parent),
                name: LineEdit::new(),
                focus: VaultCreateFormFocus::Name,
                error: None,
            });
        }
        CardChoice::Clone => {
            let default_parent = std::env::var("HOME").ok().unwrap_or_else(|| ".".into());
            *screen = Screen::Clone(VaultCloneFormState {
                url: LineEdit::new(),
                parent: LineEdit::from_str(default_parent),
                focus: VaultCloneFormFocus::Url,
                error: None,
            });
        }
        CardChoice::Open => {
            let start = std::env::var("HOME")
                .ok()
                .map(PathBuf::from)
                .filter(|p| p.is_dir())
                .unwrap_or_else(|| PathBuf::from("."));
            let canonical = match start.canonicalize() {
                Ok(p) => p,
                Err(_) => start,
            };
            let entries = helpers::read_dir_entries(&canonical).unwrap_or_default();
            *screen = Screen::Open(VaultOpenPickerState {
                cwd: canonical,
                entries,
                selected: 0,
            });
        }
    }
}

fn handle_create(screen: &mut Screen, key: KeyEvent) -> Outcome {
    let Screen::Create(state) = screen else {
        return Outcome::Continue;
    };
    match key.code {
        KeyCode::Esc => {
            *screen = Screen::Cards;
            Outcome::Continue
        }
        KeyCode::Tab | KeyCode::Down => {
            state.focus = state.focus.next();
            Outcome::Continue
        }
        KeyCode::BackTab | KeyCode::Up => {
            state.focus = state.focus.prev();
            Outcome::Continue
        }
        KeyCode::Enter => {
            let parent_raw = state.parent.as_str().to_string();
            let name_raw = state.name.as_str().to_string();
            match helpers::submit_create(&parent_raw, &name_raw) {
                Ok(dest) => Outcome::Activated(dest),
                Err(msg) => {
                    state.error = Some(msg);
                    Outcome::Continue
                }
            }
        }
        KeyCode::Backspace => {
            field_mut_create(state).delete_before();
            Outcome::Continue
        }
        KeyCode::Delete => {
            field_mut_create(state).delete_after();
            Outcome::Continue
        }
        KeyCode::Left => {
            field_mut_create(state).move_left();
            Outcome::Continue
        }
        KeyCode::Right => {
            field_mut_create(state).move_right();
            Outcome::Continue
        }
        KeyCode::Home => {
            field_mut_create(state).move_home();
            Outcome::Continue
        }
        KeyCode::End => {
            field_mut_create(state).move_end();
            Outcome::Continue
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            field_mut_create(state).insert_char(c);
            state.error = None;
            Outcome::Continue
        }
        _ => Outcome::Continue,
    }
}

fn field_mut_create(state: &mut VaultCreateFormState) -> &mut LineEdit {
    match state.focus {
        VaultCreateFormFocus::Parent => &mut state.parent,
        VaultCreateFormFocus::Name => &mut state.name,
    }
}

fn handle_clone(screen: &mut Screen, key: KeyEvent) -> Outcome {
    let Screen::Clone(state) = screen else {
        return Outcome::Continue;
    };
    match key.code {
        KeyCode::Esc => {
            *screen = Screen::Cards;
            Outcome::Continue
        }
        KeyCode::Tab | KeyCode::Down => {
            state.focus = state.focus.next();
            Outcome::Continue
        }
        KeyCode::BackTab | KeyCode::Up => {
            state.focus = state.focus.prev();
            Outcome::Continue
        }
        KeyCode::Enter => {
            let url_raw = state.url.as_str().to_string();
            let parent_raw = state.parent.as_str().to_string();
            match helpers::submit_clone(&url_raw, &parent_raw) {
                Ok(dest) => Outcome::Activated(dest),
                Err(msg) => {
                    state.error = Some(msg);
                    Outcome::Continue
                }
            }
        }
        KeyCode::Backspace => {
            field_mut_clone(state).delete_before();
            Outcome::Continue
        }
        KeyCode::Delete => {
            field_mut_clone(state).delete_after();
            Outcome::Continue
        }
        KeyCode::Left => {
            field_mut_clone(state).move_left();
            Outcome::Continue
        }
        KeyCode::Right => {
            field_mut_clone(state).move_right();
            Outcome::Continue
        }
        KeyCode::Home => {
            field_mut_clone(state).move_home();
            Outcome::Continue
        }
        KeyCode::End => {
            field_mut_clone(state).move_end();
            Outcome::Continue
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            field_mut_clone(state).insert_char(c);
            state.error = None;
            Outcome::Continue
        }
        _ => Outcome::Continue,
    }
}

fn field_mut_clone(state: &mut VaultCloneFormState) -> &mut LineEdit {
    match state.focus {
        VaultCloneFormFocus::Url => &mut state.url,
        VaultCloneFormFocus::Parent => &mut state.parent,
    }
}

fn handle_open(screen: &mut Screen, key: KeyEvent) -> Outcome {
    let Screen::Open(state) = screen else {
        return Outcome::Continue;
    };
    match key.code {
        KeyCode::Esc => {
            *screen = Screen::Cards;
            Outcome::Continue
        }
        KeyCode::Up | KeyCode::Char('k') => {
            move_cursor(state, -1);
            Outcome::Continue
        }
        KeyCode::Down | KeyCode::Char('j') => {
            move_cursor(state, 1);
            Outcome::Continue
        }
        KeyCode::Backspace | KeyCode::Char('h') => {
            navigate_to(state, state.cwd.parent().map(|p| p.to_path_buf()));
            Outcome::Continue
        }
        KeyCode::Enter | KeyCode::Char('l') => {
            let Some(entry) = state.entries.get(state.selected).cloned() else {
                return Outcome::Continue;
            };
            match entry.kind {
                VaultOpenEntryKind::Parent => {
                    navigate_to(state, state.cwd.parent().map(|p| p.to_path_buf()));
                    Outcome::Continue
                }
                VaultOpenEntryKind::Directory => {
                    let target = state.cwd.join(&entry.name);
                    navigate_to(state, Some(target));
                    Outcome::Continue
                }
                VaultOpenEntryKind::Vault => {
                    let target = state.cwd.join(&entry.name);
                    Outcome::Activated(target)
                }
            }
        }
        _ => Outcome::Continue,
    }
}

fn move_cursor(state: &mut VaultOpenPickerState, delta: i32) {
    if state.entries.is_empty() {
        return;
    }
    let last = state.entries.len() as i64 - 1;
    let next = (state.selected as i64)
        .saturating_add(delta as i64)
        .clamp(0, last);
    state.selected = next as usize;
}

fn navigate_to(state: &mut VaultOpenPickerState, target: Option<PathBuf>) {
    let Some(target) = target else { return };
    let canonical = match target.canonicalize() {
        Ok(p) => p,
        Err(_) => return,
    };
    if !canonical.is_dir() {
        return;
    }
    let entries = match helpers::read_dir_entries(&canonical) {
        Ok(es) => es,
        Err(_) => return,
    };
    state.cwd = canonical;
    state.entries = entries;
    state.selected = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }
    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    #[test]
    fn cards_arrow_keys_navigate_circularly() {
        let mut s = BootstrapState::new();
        assert_eq!(s.selected_card, CardChoice::Open);
        s.handle_key(key(KeyCode::Right));
        assert_eq!(s.selected_card, CardChoice::Clone);
        s.handle_key(key(KeyCode::Right));
        assert_eq!(s.selected_card, CardChoice::Create);
        s.handle_key(key(KeyCode::Right));
        assert_eq!(s.selected_card, CardChoice::Open);
        s.handle_key(key(KeyCode::Left));
        assert_eq!(s.selected_card, CardChoice::Create);
    }

    #[test]
    fn cards_esc_quits() {
        let mut s = BootstrapState::new();
        assert_eq!(s.handle_key(key(KeyCode::Esc)), Outcome::Quit);
    }

    #[test]
    fn cards_ctrl_c_quits_from_any_screen() {
        let mut s = BootstrapState::new();
        // Open create screen first
        s.handle_key(key(KeyCode::Char('n')));
        assert!(matches!(s.screen, Screen::Create(_)));
        assert_eq!(s.handle_key(ctrl('c')), Outcome::Quit);
    }

    #[test]
    fn enter_on_create_card_opens_create_screen() {
        let mut s = BootstrapState::new();
        s.selected_card = CardChoice::Create;
        s.handle_key(key(KeyCode::Enter));
        assert!(matches!(s.screen, Screen::Create(_)));
    }

    #[test]
    fn shortcut_n_opens_create_directly() {
        let mut s = BootstrapState::new();
        s.handle_key(key(KeyCode::Char('n')));
        assert!(matches!(s.screen, Screen::Create(_)));
        assert_eq!(s.selected_card, CardChoice::Create);
    }

    #[test]
    fn shortcut_g_opens_clone_screen() {
        let mut s = BootstrapState::new();
        s.handle_key(key(KeyCode::Char('g')));
        assert!(matches!(s.screen, Screen::Clone(_)));
        assert_eq!(s.selected_card, CardChoice::Clone);
    }

    #[test]
    fn shortcut_o_opens_open_picker() {
        let mut s = BootstrapState::new();
        s.handle_key(key(KeyCode::Char('o')));
        assert!(matches!(s.screen, Screen::Open(_)));
    }

    #[test]
    fn create_esc_returns_to_cards() {
        let mut s = BootstrapState::new();
        s.handle_key(key(KeyCode::Char('n')));
        s.handle_key(key(KeyCode::Esc));
        assert!(matches!(s.screen, Screen::Cards));
    }

    #[test]
    fn create_tab_cycles_focus() {
        let mut s = BootstrapState::new();
        s.handle_key(key(KeyCode::Char('n')));
        if let Screen::Create(state) = &s.screen {
            assert_eq!(state.focus, VaultCreateFormFocus::Name);
        }
        s.handle_key(key(KeyCode::Tab));
        if let Screen::Create(state) = &s.screen {
            assert_eq!(state.focus, VaultCreateFormFocus::Parent);
        }
    }

    #[test]
    fn create_char_input_lands_in_focused_field() {
        let mut s = BootstrapState::new();
        s.handle_key(key(KeyCode::Char('n')));
        // Focus starts on Name
        s.handle_key(key(KeyCode::Char('v')));
        s.handle_key(key(KeyCode::Char('1')));
        if let Screen::Create(state) = &s.screen {
            assert_eq!(state.name.as_str(), "v1");
        } else {
            panic!("expected Create");
        }
    }

    #[test]
    fn create_backspace_deletes_in_focused_field() {
        let mut s = BootstrapState::new();
        s.handle_key(key(KeyCode::Char('n')));
        s.handle_key(key(KeyCode::Char('a')));
        s.handle_key(key(KeyCode::Char('b')));
        s.handle_key(key(KeyCode::Backspace));
        if let Screen::Create(state) = &s.screen {
            assert_eq!(state.name.as_str(), "a");
        }
    }

    #[test]
    fn create_empty_name_shows_inline_error() {
        let mut s = BootstrapState::new();
        s.handle_key(key(KeyCode::Char('n')));
        // No characters typed for name; submit
        let out = s.handle_key(key(KeyCode::Enter));
        assert_eq!(out, Outcome::Continue);
        if let Screen::Create(state) = &s.screen {
            assert!(state.error.as_deref().unwrap_or("").contains("name"));
        }
    }

    #[test]
    fn create_successful_submit_returns_activated() {
        // Use a real tempdir as parent + a fresh vault name.
        let parent = tempfile::TempDir::new().unwrap();
        std::env::set_var("HOME", parent.path());
        let mut s = BootstrapState::new();
        s.handle_key(key(KeyCode::Char('n')));
        // Type name
        for c in "fresh-vault".chars() {
            s.handle_key(key(KeyCode::Char(c)));
        }
        let out = s.handle_key(key(KeyCode::Enter));
        match out {
            Outcome::Activated(path) => {
                assert!(path.starts_with(parent.path()));
                assert!(path.ends_with("fresh-vault"));
            }
            other => panic!("expected Activated, got {other:?}"),
        }
    }

    #[test]
    fn clone_empty_url_shows_inline_error() {
        let mut s = BootstrapState::new();
        s.handle_key(key(KeyCode::Char('g')));
        let out = s.handle_key(key(KeyCode::Enter));
        assert_eq!(out, Outcome::Continue);
        if let Screen::Clone(state) = &s.screen {
            assert!(state.error.as_deref().unwrap_or("").contains("URL"));
        }
    }

    #[test]
    fn clone_char_input_lands_in_url_field_by_default() {
        let mut s = BootstrapState::new();
        s.handle_key(key(KeyCode::Char('g')));
        for c in "https://x.git".chars() {
            s.handle_key(key(KeyCode::Char(c)));
        }
        if let Screen::Clone(state) = &s.screen {
            assert_eq!(state.url.as_str(), "https://x.git");
        }
    }

    #[test]
    fn open_screen_descends_into_directory_and_returns_to_parent() {
        let root = tempfile::TempDir::new().unwrap();
        let child = root.path().join("inner");
        std::fs::create_dir(&child).unwrap();
        std::env::set_var("HOME", root.path());

        let mut s = BootstrapState::new();
        s.handle_key(key(KeyCode::Char('o')));
        // The first non-Parent entry is `inner` (the only dir).
        if let Screen::Open(state) = &s.screen {
            let inner_idx = state
                .entries
                .iter()
                .position(|e| e.name == "inner")
                .expect("inner listed");
            assert!(inner_idx > 0); // Parent is at 0
        }

        // Move cursor down past the `..` to `inner` and Enter
        s.handle_key(key(KeyCode::Down));
        s.handle_key(key(KeyCode::Enter));
        if let Screen::Open(state) = &s.screen {
            assert!(state.cwd.ends_with("inner"));
        } else {
            panic!("expected Open");
        }

        // Backspace ascends
        s.handle_key(key(KeyCode::Backspace));
        if let Screen::Open(state) = &s.screen {
            assert!(!state.cwd.ends_with("inner"));
        }
    }

    #[test]
    fn open_screen_enter_on_vault_returns_activated() {
        let root = tempfile::TempDir::new().unwrap();
        let vault = root.path().join("my-vault");
        std::fs::create_dir(&vault).unwrap();
        httui_core::vault_config::scaffold::scaffold_new_vault(&vault).unwrap();
        std::env::set_var("HOME", root.path());

        let mut s = BootstrapState::new();
        s.handle_key(key(KeyCode::Char('o')));
        // Walk the entries to find my-vault and select it
        if let Screen::Open(state) = &mut s.screen {
            let idx = state
                .entries
                .iter()
                .position(|e| e.name == "my-vault")
                .expect("vault listed");
            state.selected = idx;
        }
        let out = s.handle_key(key(KeyCode::Enter));
        match out {
            Outcome::Activated(p) => assert!(p.ends_with("my-vault")),
            other => panic!("expected Activated, got {other:?}"),
        }
    }

    #[test]
    fn open_screen_esc_returns_to_cards() {
        let root = tempfile::TempDir::new().unwrap();
        std::env::set_var("HOME", root.path());

        let mut s = BootstrapState::new();
        s.handle_key(key(KeyCode::Char('o')));
        s.handle_key(key(KeyCode::Esc));
        assert!(matches!(s.screen, Screen::Cards));
    }
}
