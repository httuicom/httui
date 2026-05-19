//! Tree-sidebar navigation + in-tree prompts and tab switching.
//! Mechanically split out of `crate::input::apply::misc` (tui-v2
//! vertical 1, fase 1 p6g) with no logic change — every arm body is
//! copied verbatim. The outer router (`apply_action`) routes only
//! this group's variants here, so the `unreachable!` is a
//! compile-time-backed invariant.

use crate::app::{App, StatusKind};
use crate::buffer::Cursor;
use crate::input::action::Action;
use crate::input::apply::navigation::run_tree_prompt;
use crate::tree::{TreePrompt, TreePromptKind};
use crate::vim::mode::Mode;

pub(crate) fn apply_tree_nav(app: &mut App, action: Action, _recording: bool) {
    match action {
        Action::TreeToggle => {
            if app.tree.visible {
                app.tree.visible = false;
                if app.vim.mode == Mode::Tree {
                    app.vim.enter_normal();
                }
            } else {
                app.tree.visible = true;
                app.tree.refresh(&app.vault_path);
                app.vim.mode = Mode::Tree;
            }
        }
        Action::FocusSwap => {
            if !app.tree.visible {
                return;
            }
            if app.vim.mode == Mode::Tree {
                app.vim.enter_normal();
            } else if app.vim.mode == Mode::Normal {
                app.vim.mode = Mode::Tree;
            }
        }
        Action::TreeSelectNext => app.tree.select_next(),
        Action::TreeSelectPrev => app.tree.select_prev(),
        Action::TreeSelectFirst => app.tree.select_first(),
        Action::TreeSelectLast => app.tree.select_last(),
        Action::TreeRefresh => {
            let vault = app.vault_path.clone();
            app.tree.refresh(&vault);
        }
        Action::TreeCollapse => {
            if app.tree.collapse_parent() {
                let vault = app.vault_path.clone();
                app.tree.refresh(&vault);
            }
        }
        Action::TreeActivate => {
            let Some(node) = app.tree.current().cloned() else {
                return;
            };
            if node.is_dir {
                if app.tree.toggle_expand() {
                    let vault = app.vault_path.clone();
                    app.tree.refresh(&vault);
                }
            } else {
                // Tree-driven open mirrors the modal: every Enter opens
                // a new tab (or switches to an existing one). Use `:e
                // <path>` if you want the vim-style replace behavior.
                let path = std::path::PathBuf::from(&node.path);
                match app.open_in_new_tab(path) {
                    Ok(msg) => {
                        app.set_status(StatusKind::Info, msg);
                        // Hand focus back to the editor on successful open —
                        // matches how netrw exits the tree after Enter.
                        app.vim.enter_normal();
                    }
                    Err(msg) => app.set_status(StatusKind::Error, msg),
                }
            }
        }
        Action::TabNext => {
            // When the cursor sits on a result row, `gt` cycles
            // the result-panel tab (Result → Messages → Plan →
            // Stats → Result) instead of switching editor tabs —
            // the editor-tab swap wouldn't be useful from inside a
            // table, and the result-panel needs *some* keyboard
            // affordance.
            if matches!(
                app.document().map(|d| d.cursor()),
                Some(Cursor::InBlockResult { .. })
            ) {
                app.db_result_tab = app.db_result_tab.next();
            } else {
                app.next_tab();
                app.refresh_viewport_for_cursor();
            }
        }
        Action::TabPrev => {
            if matches!(
                app.document().map(|d| d.cursor()),
                Some(Cursor::InBlockResult { .. })
            ) {
                app.db_result_tab = app.db_result_tab.prev();
            } else {
                app.prev_tab();
                app.refresh_viewport_for_cursor();
            }
        }
        Action::TabGoto(n) => {
            app.goto_tab(n);
            app.refresh_viewport_for_cursor();
        }
        Action::TreeCreate => {
            // Open the in-tree prompt anchored to the selected folder
            // (or the parent of the selected file). The user types
            // either a filename (e.g. `notes.md`) or a name with
            // trailing `/` (e.g. `subdir/`) to make a folder.
            let dir = match app.tree.current() {
                Some(node) if node.is_dir => node.path.clone(),
                Some(node) => match std::path::Path::new(&node.path).parent() {
                    Some(p) if !p.as_os_str().is_empty() => p.display().to_string(),
                    _ => String::new(),
                },
                None => String::new(),
            };
            app.tree.prompt = Some(TreePrompt::new(
                TreePromptKind::Create { dir },
                String::new(),
            ));
            app.vim.mode = Mode::TreePrompt;
        }
        Action::TreeRename => {
            let Some(node) = app.tree.current() else {
                return;
            };
            // Pre-fill the buffer with the source path so the user
            // edits the destination in place. Allowed for files and
            // folders alike — `rename_path` handles both.
            let path = node.path.clone();
            app.tree.prompt = Some(TreePrompt::new(
                TreePromptKind::Rename { from: path.clone() },
                path,
            ));
            app.vim.mode = Mode::TreePrompt;
        }
        Action::TreeDelete => {
            let Some(node) = app.tree.current() else {
                return;
            };
            app.tree.prompt = Some(TreePrompt::new(
                TreePromptKind::Delete {
                    target: node.path.clone(),
                },
                String::new(),
            ));
            app.vim.mode = Mode::TreePrompt;
        }
        Action::TreePromptChar(c) => {
            if let Some(prompt) = app.tree.prompt.as_mut() {
                prompt.input.insert_char(c);
            }
        }
        Action::TreePromptBackspace => {
            if let Some(prompt) = app.tree.prompt.as_mut() {
                if !prompt.input.delete_before() {
                    // Empty buffer + backspace acts like cancel.
                    app.tree.prompt = None;
                    app.vim.mode = Mode::Tree;
                }
            } else {
                app.vim.mode = Mode::Tree;
            }
        }
        Action::TreePromptDelete => {
            if let Some(prompt) = app.tree.prompt.as_mut() {
                prompt.input.delete_after();
            }
        }
        Action::TreePromptCursorLeft => {
            if let Some(prompt) = app.tree.prompt.as_mut() {
                prompt.input.move_left();
            }
        }
        Action::TreePromptCursorRight => {
            if let Some(prompt) = app.tree.prompt.as_mut() {
                prompt.input.move_right();
            }
        }
        Action::TreePromptCursorHome => {
            if let Some(prompt) = app.tree.prompt.as_mut() {
                prompt.input.move_home();
            }
        }
        Action::TreePromptCursorEnd => {
            if let Some(prompt) = app.tree.prompt.as_mut() {
                prompt.input.move_end();
            }
        }
        Action::TreePromptCancel => {
            app.tree.prompt = None;
            app.vim.mode = Mode::Tree;
        }
        Action::TreePromptExecute => {
            let Some(prompt) = app.tree.prompt.take() else {
                app.vim.mode = Mode::Tree;
                return;
            };
            app.vim.mode = Mode::Tree;
            run_tree_prompt(app, prompt);
        }
        _ => unreachable!("apply_tree_nav: variante fora do grupo"),
    }
}
