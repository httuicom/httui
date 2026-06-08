use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

mod dispatch;
use dispatch::Action;

const TUI_BIN: &str = "httui-tui";
#[cfg(not(target_os = "macos"))]
const DESKTOP_BIN: &str = "httui-desktop";
#[cfg(target_os = "macos")]
const MACOS_APP: &str = "httui";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match Action::from_args(&args) {
        Action::Help => {
            print_help();
            ExitCode::SUCCESS
        }
        Action::Tui(forwarded) => run_tui(&forwarded),
        Action::Desktop(forwarded) => run_desktop(&forwarded),
    }
}

fn run_tui(args: &[String]) -> ExitCode {
    let Some(dir) = sibling_dir() else {
        return fail("could not resolve installation directory");
    };
    let bin = dir.join(with_exe_suffix(TUI_BIN));
    if !bin.exists() {
        return fail(&format!(
            "{} not found next to launcher (looked at {}).\n\
             In development, run `cargo run -p httui-tui` instead.",
            TUI_BIN,
            bin.display()
        ));
    }
    spawn(Command::new(&bin).args(args))
}

#[cfg(target_os = "macos")]
fn run_desktop(args: &[String]) -> ExitCode {
    // `open -a` goes through LaunchServices so the app is registered
    // in the Dock and gets the normal app lifecycle. Spawning the
    // Mach-O directly works but the window can fail to focus and the
    // process is parented to the terminal.
    //
    // `HTTUI_DESKTOP_APP_NAME` is an integration-test escape hatch: it
    // lets tests target a guaranteed-missing app so the launcher's
    // spawn path runs without actually opening the user's real
    // /Applications/httui.app.
    let app = env::var("HTTUI_DESKTOP_APP_NAME").unwrap_or_else(|_| MACOS_APP.to_string());
    let mut cmd = Command::new("open");
    cmd.arg("-a").arg(&app);
    if !args.is_empty() {
        cmd.arg("--args").args(args);
    }
    spawn(&mut cmd)
}

#[cfg(not(target_os = "macos"))]
fn run_desktop(args: &[String]) -> ExitCode {
    let Some(dir) = sibling_dir() else {
        return fail("could not resolve installation directory");
    };
    let bin = dir.join(with_exe_suffix(DESKTOP_BIN));
    if !bin.exists() {
        return fail(&format!(
            "{} not found next to launcher (looked at {}).",
            DESKTOP_BIN,
            bin.display()
        ));
    }
    spawn(Command::new(&bin).args(args))
}

fn sibling_dir() -> Option<PathBuf> {
    env::current_exe()
        .ok()
        .as_deref()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
}

#[cfg(windows)]
fn with_exe_suffix(name: &str) -> String {
    format!("{name}.exe")
}

#[cfg(not(windows))]
fn with_exe_suffix(name: &str) -> String {
    name.to_string()
}

fn spawn(cmd: &mut Command) -> ExitCode {
    match cmd.status() {
        Ok(status) => ExitCode::from(u8::try_from(status.code().unwrap_or(1)).unwrap_or(1)),
        Err(e) => fail(&format!("failed to launch: {e}")),
    }
}

fn fail(msg: &str) -> ExitCode {
    eprintln!("httui: {msg}");
    ExitCode::from(2)
}

fn print_help() {
    println!(
        "httui — terminal + desktop launcher\n\
         \n\
         USAGE:\n    \
             httui [TUI_ARGS...]          Open the terminal UI (default)\n    \
             httui desktop [APP_ARGS...]  Open the desktop application\n    \
             httui --help                 Show this message\n    \
             httui --version              Show launcher version\n\
         \n\
         Any other flags or positional arguments are forwarded to the TUI.\n\
         For TUI-specific help, invoke the binary directly: `httui-tui --help`."
    );
}
