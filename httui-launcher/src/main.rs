use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

mod dispatch;
use dispatch::{resolve_sibling, Action};

const TUI_BIN: &str = "httui-tui";
#[cfg(not(target_os = "macos"))]
const DESKTOP_BIN: &str = "httui-desktop";
#[cfg(target_os = "macos")]
const MACOS_APP: &str = "httui";

#[cfg(windows)]
const EXE_SUFFIX: &str = ".exe";
#[cfg(not(windows))]
const EXE_SUFFIX: &str = "";

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

// `HTTUI_TUI_BIN` / `HTTUI_DESKTOP_BIN` are integration-test escape
// hatches that let tests point the launcher at a known-missing path
// without copying the launcher itself anywhere (which trips Linux's
// ETXTBSY when the freshly-copied file is execve'd).
fn lookup_sibling(name: &'static str, env_override: &str) -> Result<PathBuf, String> {
    let override_val = env::var(env_override).ok();
    let dir = sibling_dir();
    resolve_sibling(
        name,
        override_val.as_deref(),
        dir.as_deref(),
        |p| p.exists(),
        EXE_SUFFIX,
    )
}

fn run_tui(args: &[String]) -> ExitCode {
    let bin = match lookup_sibling(TUI_BIN, "HTTUI_TUI_BIN") {
        Ok(p) => p,
        Err(msg) => return fail(&msg),
    };
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
    let bin = match lookup_sibling(DESKTOP_BIN, "HTTUI_DESKTOP_BIN") {
        Ok(p) => p,
        Err(msg) => return fail(&msg),
    };
    spawn(Command::new(&bin).args(args))
}

fn sibling_dir() -> Option<PathBuf> {
    // current_exe() returns the symlink path itself on macOS (and on
    // Windows when invoked through a junction), so resolving it first
    // is required for `httui` to find its siblings when installed via
    // a symlinked shim — e.g. `brew install --cask httui` symlinking
    // /usr/local/bin/httui → /Applications/httui.app/Contents/MacOS/httui.
    let exe = env::current_exe().ok()?;
    let resolved = std::fs::canonicalize(&exe).unwrap_or(exe);
    resolved.parent().map(Path::to_path_buf)
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
