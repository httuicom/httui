//! `impl App` — file open + tab management.
//!
//! Mechanically extracted from the monolithic `impl App` in `app.rs`
//! (tui-v2 vertical 1, fase 2 p2-file_tab) — pure code move, no
//! behavior change. Sibling `impl App {}` block; methods stay `pub fn`
//! so every `app.foo()` call site keeps resolving unchanged.

use std::path::PathBuf;

use crate::document_loader;
use crate::pane::{Pane, TabState};

use super::{file_name, tab_has_dirty, App};

impl App {
    // ----- file open / tab management ------------------------------------

    /// Replace the focused pane's document with the file at
    /// `relative_path`. If that file is the focused leaf of another
    /// tab, switches to that tab instead. Refuses to clobber a dirty
    /// buffer unless `force` is true.
    pub fn open_document(&mut self, relative_path: PathBuf, force: bool) -> Result<String, String> {
        if let Some(idx) = self.tabs.find_focused(&relative_path) {
            self.tabs.active = idx;
            return Ok(format!("\"{}\"", file_name(&relative_path)));
        }
        if !force
            && self
                .active_pane()
                .and_then(|p| p.document.as_ref())
                .is_some_and(|d| d.is_dirty())
        {
            return Err("no write since last change (add ! to override)".into());
        }
        let doc = document_loader::load_document(&self.vault_path, &relative_path)
            .map_err(|e| format!("E484: Can't open file: {e}"))?;
        let name = file_name(&relative_path);
        // No tab yet (e.g. last close left us empty)? Open as new tab.
        if self.tabs.is_empty() {
            self.tabs
                .tabs
                .push(TabState::new(Pane::new(doc, relative_path)));
            self.tabs.active = 0;
            return Ok(format!("\"{name}\""));
        }
        // Replace the focused pane's document in-place.
        if let Some(p) = self.active_pane_mut() {
            p.document = Some(doc);
            p.document_path = Some(relative_path);
            p.viewport_top = 0;
        }
        Ok(format!("\"{name}\""))
    }

    /// Open `relative_path` in a brand-new tab. If already focused in
    /// another tab, switches to it instead.
    pub fn open_in_new_tab(&mut self, relative_path: PathBuf) -> Result<String, String> {
        if let Some(idx) = self.tabs.find_focused(&relative_path) {
            self.tabs.active = idx;
            return Ok(format!("\"{}\"", file_name(&relative_path)));
        }
        let doc = document_loader::load_document(&self.vault_path, &relative_path)
            .map_err(|e| format!("E484: Can't open file: {e}"))?;
        let name = file_name(&relative_path);
        let new_tab = TabState::new(Pane::new(doc, relative_path));
        self.tabs.tabs.push(new_tab);
        self.tabs.active = self.tabs.tabs.len() - 1;
        Ok(format!("\"{name}\""))
    }

    pub fn next_tab(&mut self) {
        if self.tabs.len() <= 1 {
            return;
        }
        self.tabs.active = (self.tabs.active + 1) % self.tabs.len();
    }

    pub fn prev_tab(&mut self) {
        if self.tabs.len() <= 1 {
            return;
        }
        self.tabs.active = if self.tabs.active == 0 {
            self.tabs.len() - 1
        } else {
            self.tabs.active - 1
        };
    }

    /// Switch to the 1-indexed tab number `n`. Out-of-range no-ops.
    pub fn goto_tab(&mut self, n: usize) {
        if n == 0 || n > self.tabs.len() {
            return;
        }
        self.tabs.active = n - 1;
    }

    /// Close the active tab (drops every pane inside it). With dirty
    /// content in any pane and `force == false`, refuses.
    pub fn close_tab(&mut self, force: bool) -> Result<String, String> {
        if self.tabs.is_empty() {
            return Err("no tab to close".into());
        }
        let active = self.tabs.active;
        if !force && tab_has_dirty(&self.tabs.tabs[active]) {
            return Err("no write since last change (add ! to override)".into());
        }
        let removed = self.tabs.tabs.remove(active);
        let removed_path = removed
            .active_leaf()
            .document_path
            .clone()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(no name)".into());
        if self.tabs.tabs.is_empty() {
            self.tabs.active = 0;
            return Ok(format!("closed \"{removed_path}\""));
        }
        if active >= self.tabs.tabs.len() {
            self.tabs.active = self.tabs.tabs.len() - 1;
        }
        Ok(format!("closed \"{removed_path}\""))
    }
}
