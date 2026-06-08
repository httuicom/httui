use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Tui(Vec<String>),
    Desktop(Vec<String>),
    Help,
}

impl Action {
    pub fn from_args(args: &[String]) -> Action {
        match args.first().map(String::as_str) {
            Some("--help") | Some("-h") => Action::Help,
            Some("desktop") => Action::Desktop(args[1..].to_vec()),
            _ => Action::Tui(args.to_vec()),
        }
    }
}

/// Pure sibling-binary resolution: tests pass in the env override value
/// (None when unset) and the installation directory; production wires
/// them to `env::var(...)` and `current_exe().parent()`.
pub fn resolve_sibling(
    name: &str,
    env_override: Option<&str>,
    sibling_dir: Option<&Path>,
    exists: impl Fn(&Path) -> bool,
    suffix: &str,
) -> Result<PathBuf, String> {
    if let Some(p) = env_override {
        return Ok(PathBuf::from(p));
    }
    let dir = sibling_dir.ok_or_else(|| "could not resolve installation directory".to_string())?;
    let bin = dir.join(format!("{name}{suffix}"));
    if !exists(&bin) {
        return Err(format!(
            "{name} not found next to launcher (looked at {}).\n\
             In development, run `cargo run -p httui-tui` instead.",
            bin.display()
        ));
    }
    Ok(bin)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(slice: &[&str]) -> Vec<String> {
        slice.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn no_args_runs_tui_with_empty_forward() {
        assert_eq!(Action::from_args(&[]), Action::Tui(vec![]));
    }

    #[test]
    fn desktop_subcommand_routes_to_desktop() {
        assert_eq!(
            Action::from_args(&args(&["desktop"])),
            Action::Desktop(vec![])
        );
    }

    #[test]
    fn desktop_forwards_remaining_args() {
        assert_eq!(
            Action::from_args(&args(&["desktop", "--debug", "path/to/vault"])),
            Action::Desktop(args(&["--debug", "path/to/vault"]))
        );
    }

    #[test]
    fn help_short_and_long_flags() {
        assert_eq!(Action::from_args(&args(&["--help"])), Action::Help);
        assert_eq!(Action::from_args(&args(&["-h"])), Action::Help);
    }

    #[test]
    fn tui_args_are_forwarded_verbatim() {
        assert_eq!(
            Action::from_args(&args(&["--log-level", "debug", "--data-dir", "/tmp/x"])),
            Action::Tui(args(&["--log-level", "debug", "--data-dir", "/tmp/x"]))
        );
    }

    #[test]
    fn help_takes_precedence_only_when_first() {
        assert_eq!(
            Action::from_args(&args(&["--log-level", "--help"])),
            Action::Tui(args(&["--log-level", "--help"]))
        );
    }

    #[test]
    fn desktop_must_be_first_token() {
        assert_eq!(
            Action::from_args(&args(&["--flag", "desktop"])),
            Action::Tui(args(&["--flag", "desktop"]))
        );
    }

    #[test]
    fn resolve_sibling_prefers_env_override() {
        let got = resolve_sibling(
            "httui-tui",
            Some("/custom/path/to/tui"),
            Some(Path::new("/sibling/dir")),
            |_| false,
            "",
        )
        .unwrap();
        assert_eq!(got, PathBuf::from("/custom/path/to/tui"));
    }

    #[test]
    fn resolve_sibling_fails_when_sibling_dir_missing() {
        let err = resolve_sibling("httui-tui", None, None, |_| true, "").expect_err("should fail");
        assert!(err.contains("installation directory"), "{err}");
    }

    #[test]
    fn resolve_sibling_fails_when_binary_does_not_exist() {
        let err = resolve_sibling(
            "httui-tui",
            None,
            Some(Path::new("/some/dir")),
            |_| false,
            "",
        )
        .expect_err("should fail");
        assert!(err.contains("httui-tui not found"), "{err}");
        assert!(err.contains("/some/dir/httui-tui"), "{err}");
    }

    #[test]
    fn resolve_sibling_returns_path_when_binary_exists() {
        let got = resolve_sibling(
            "httui-tui",
            None,
            Some(Path::new("/some/dir")),
            |_| true,
            "",
        )
        .unwrap();
        assert_eq!(got, PathBuf::from("/some/dir/httui-tui"));
    }

    #[test]
    fn resolve_sibling_applies_exe_suffix() {
        let got = resolve_sibling(
            "httui-tui",
            None,
            Some(Path::new("C:\\bin")),
            |_| true,
            ".exe",
        )
        .unwrap();
        assert!(got.to_string_lossy().ends_with("httui-tui.exe"), "{got:?}");
    }
}
