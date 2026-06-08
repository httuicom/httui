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
}
