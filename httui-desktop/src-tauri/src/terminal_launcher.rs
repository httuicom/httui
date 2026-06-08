// Auto-provisions the `httui` terminal launcher in the user's PATH.
//
// Tauri's auto-updater replaces the .app bundle in-place but cannot
// touch /usr/local/bin — that lives outside the app's sandbox. Users
// who installed via the Homebrew cask got the symlink from the cask
// recipe; users who installed via .dmg or upgraded across the launcher
// introduction (0.4.1 → 0.4.2) end up with the launcher binary inside
// the app but nothing pointing at it on PATH.
//
// This module fixes that by linking the launcher into ~/.local/bin/
// the first time the desktop runs from a bundle that has the launcher
// next to it. ~/.local/bin is writable without sudo and is on the
// default PATH for modern zsh/bash. /usr/local/bin would require root
// and the cask already covers that path.
//
// The decision logic lives in `decide` (pure, fully unit tested);
// `apply` performs the filesystem effects.

use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
pub fn ensure_launcher_in_path() {
    if let Some(action) = decide_from_env() {
        if let Err(e) = apply(&action) {
            eprintln!("[terminal-launcher] {e}");
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn ensure_launcher_in_path() {
    // Linux installs already place /usr/bin/httui via the .deb/.rpm
    // postinst step; Windows PATH wiring is tracked separately.
}

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Symlink { src: PathBuf, dst: PathBuf },
}

#[cfg(target_os = "macos")]
fn decide_from_env() -> Option<Action> {
    let exe = std::env::current_exe().ok()?;
    let resolved = std::fs::canonicalize(&exe).unwrap_or(exe);
    let path = std::env::var("PATH").unwrap_or_default();
    let home = std::env::var("HOME").ok()?;
    decide(
        &resolved,
        &path,
        Path::new(&home),
        |p| p.exists(),
        |p| std::fs::canonicalize(p).ok(),
    )
}

/// Pure decision: returns None when no action is needed (sibling
/// missing, or PATH already contains a `httui` resolving to our
/// launcher), otherwise returns the desired Symlink. All filesystem
/// observations are injected via closures so tests can simulate.
pub fn decide(
    resolved_exe: &Path,
    path_var: &str,
    home: &Path,
    exists: impl Fn(&Path) -> bool,
    canonicalize: impl Fn(&Path) -> Option<PathBuf>,
) -> Option<Action> {
    let bundle_dir = resolved_exe.parent()?;
    let launcher = bundle_dir.join("httui");
    if !exists(&launcher) {
        return None;
    }

    for dir in path_var.split(':').filter(|s| !s.is_empty()) {
        let candidate = PathBuf::from(dir).join("httui");
        if !exists(&candidate) {
            continue;
        }
        if let Some(c) = canonicalize(&candidate) {
            if c == launcher {
                return None;
            }
        }
    }

    let dst = home.join(".local/bin/httui");
    Some(Action::Symlink { src: launcher, dst })
}

#[cfg(target_os = "macos")]
fn apply(action: &Action) -> Result<(), String> {
    use std::os::unix::fs::symlink;
    let Action::Symlink { src, dst } = action;
    let parent = dst.parent().ok_or("symlink target has no parent")?;
    std::fs::create_dir_all(parent).map_err(|e| format!("mkdir -p {}: {e}", parent.display()))?;
    if dst.symlink_metadata().is_ok() {
        std::fs::remove_file(dst).map_err(|e| format!("rm {}: {e}", dst.display()))?;
    }
    symlink(src, dst)
        .map_err(|e| format!("symlink {} -> {}: {e}", dst.display(), src.display()))?;
    eprintln!(
        "[terminal-launcher] linked {} -> {}",
        dst.display(),
        src.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn no_sibling_launcher_returns_none() {
        let action = decide(
            &p("/Applications/httui.app/Contents/MacOS/httui-desktop"),
            "/usr/bin:/usr/local/bin",
            &p("/Users/x"),
            |_| false,
            |_| None,
        );
        assert_eq!(action, None);
    }

    #[test]
    fn path_already_points_to_our_launcher_returns_none() {
        let launcher = p("/Applications/httui.app/Contents/MacOS/httui");
        let action = decide(
            &p("/Applications/httui.app/Contents/MacOS/httui-desktop"),
            "/usr/local/bin:/usr/bin",
            &p("/Users/x"),
            |path| path == launcher || path == p("/usr/local/bin/httui"),
            |path| {
                if path == p("/usr/local/bin/httui") {
                    Some(launcher.clone())
                } else {
                    None
                }
            },
        );
        assert_eq!(action, None);
    }

    #[test]
    fn missing_path_link_returns_symlink_action() {
        let launcher = p("/Applications/httui.app/Contents/MacOS/httui");
        let action = decide(
            &p("/Applications/httui.app/Contents/MacOS/httui-desktop"),
            "/usr/local/bin:/usr/bin",
            &p("/Users/x"),
            |path| path == launcher,
            |_| None,
        );
        assert_eq!(
            action,
            Some(Action::Symlink {
                src: launcher,
                dst: p("/Users/x/.local/bin/httui"),
            })
        );
    }

    #[test]
    fn stale_httui_pointing_elsewhere_still_links() {
        let launcher = p("/Applications/httui.app/Contents/MacOS/httui");
        let action = decide(
            &p("/Applications/httui.app/Contents/MacOS/httui-desktop"),
            "/opt/old/bin",
            &p("/Users/x"),
            |path| path == launcher || path == p("/opt/old/bin/httui"),
            |path| {
                if path == p("/opt/old/bin/httui") {
                    Some(p("/opt/old/bin/httui"))
                } else {
                    None
                }
            },
        );
        assert!(matches!(action, Some(Action::Symlink { .. })));
    }

    #[test]
    fn empty_path_var_falls_through_to_symlink() {
        let launcher = p("/Applications/httui.app/Contents/MacOS/httui");
        let action = decide(
            &p("/Applications/httui.app/Contents/MacOS/httui-desktop"),
            "",
            &p("/Users/x"),
            |path| path == launcher,
            |_| None,
        );
        assert_eq!(
            action,
            Some(Action::Symlink {
                src: launcher,
                dst: p("/Users/x/.local/bin/httui"),
            })
        );
    }

    #[test]
    fn exe_at_filesystem_root_returns_none() {
        let action = decide(
            &p("/httui-desktop"),
            "",
            &p("/Users/x"),
            |_| false,
            |_| None,
        );
        assert_eq!(action, None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn apply_creates_symlink_in_missing_parent_dir() {
        use std::fs;
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("launcher-stub");
        fs::write(&src, b"stub").unwrap();
        let dst = dir.path().join("nested/bin/httui");
        apply(&Action::Symlink {
            src: src.clone(),
            dst: dst.clone(),
        })
        .unwrap();
        assert!(dst.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(fs::read_link(&dst).unwrap(), src);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn apply_replaces_existing_symlink() {
        use std::fs;
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("new-launcher");
        fs::write(&src, b"stub").unwrap();
        let stale = dir.path().join("old-target");
        fs::write(&stale, b"stale").unwrap();
        let dst = dir.path().join("bin/httui");
        fs::create_dir_all(dst.parent().unwrap()).unwrap();
        symlink(&stale, &dst).unwrap();
        apply(&Action::Symlink {
            src: src.clone(),
            dst: dst.clone(),
        })
        .unwrap();
        assert_eq!(fs::read_link(&dst).unwrap(), src);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn apply_replaces_dangling_symlink() {
        use std::fs;
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("launcher");
        fs::write(&src, b"stub").unwrap();
        let dst = dir.path().join("bin/httui");
        fs::create_dir_all(dst.parent().unwrap()).unwrap();
        symlink(dir.path().join("does-not-exist"), &dst).unwrap();
        assert!(!dst.exists());
        assert!(dst.symlink_metadata().is_ok());
        apply(&Action::Symlink {
            src: src.clone(),
            dst: dst.clone(),
        })
        .unwrap();
        assert_eq!(fs::read_link(&dst).unwrap(), src);
    }
}
