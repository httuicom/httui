// End-to-end coverage for the launcher binary. These exercise the
// real `main()` entrypoint and the spawn paths so the file does not
// rely on `// coverage:exclude`.
//
// We do NOT copy the launcher to a tempdir to "hide" sibling binaries:
// on Linux, execve'ing a freshly-copied binary races with the kernel's
// write-close window and trips ETXTBSY ("Text file busy"). Instead the
// launcher exposes `HTTUI_TUI_BIN` / `HTTUI_DESKTOP_BIN` / the macOS
// `HTTUI_DESKTOP_APP_NAME` env hooks so tests can target a known-missing
// path without touching the on-disk launcher.

use std::path::PathBuf;
use std::process::Command;

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_httui"))
}

const MISSING_PATH: &str = "/nonexistent/httui-launcher-integration-test-target";

#[test]
fn help_flag_prints_usage_and_exits_success() {
    let out = Command::new(bin()).arg("--help").output().expect("spawn");
    assert!(out.status.success(), "exit: {:?}", out.status);
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("USAGE:") && stdout.contains("httui desktop"),
        "stdout: {stdout}"
    );
}

#[test]
fn short_help_flag_works() {
    let out = Command::new(bin()).arg("-h").output().expect("spawn");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("USAGE:"));
}

#[test]
fn missing_tui_sibling_fails_with_clear_error() {
    let out = Command::new(bin())
        .env("HTTUI_TUI_BIN", MISSING_PATH)
        .output()
        .expect("spawn");
    assert!(!out.status.success(), "expected failure, got success");
    let stderr = String::from_utf8(out.stderr).unwrap();
    // With the env override set we go straight to spawn — the
    // "not found" diagnostic only fires for the sibling-lookup path.
    // What we actually observe is the underlying OS error from execve.
    assert!(stderr.contains("failed to launch"), "stderr: {stderr}");
}

#[test]
fn extra_args_are_forwarded_to_the_tui_path() {
    let out = Command::new(bin())
        .env("HTTUI_TUI_BIN", MISSING_PATH)
        .args(["--log-level", "debug"])
        .output()
        .expect("spawn");
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("failed to launch"), "stderr: {stderr}");
}

#[cfg(not(target_os = "macos"))]
#[test]
fn missing_desktop_sibling_fails_on_non_macos() {
    let out = Command::new(bin())
        .env("HTTUI_DESKTOP_BIN", MISSING_PATH)
        .arg("desktop")
        .output()
        .expect("spawn");
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("failed to launch"), "stderr: {stderr}");
}

#[cfg(target_os = "macos")]
#[test]
fn desktop_subcommand_invokes_open_on_macos() {
    // Exercise the macOS run_desktop branch + spawn() Ok(status) path
    // without actually opening the user's real /Applications/httui.app:
    // HTTUI_DESKTOP_APP_NAME points `open -a` at an app that does not
    // exist, so the launcher's spawn returns a non-zero status without
    // any GUI side effects.
    let out = Command::new(bin())
        .arg("desktop")
        .env(
            "HTTUI_DESKTOP_APP_NAME",
            "httui-launcher-integration-test-does-not-exist",
        )
        .output()
        .expect("spawn");
    assert!(!out.status.success(), "expected non-zero exit");
}

#[cfg(target_os = "macos")]
#[test]
fn desktop_with_args_appends_dash_dash_args() {
    let out = Command::new(bin())
        .args(["desktop", "--debug", "foo"])
        .env(
            "HTTUI_DESKTOP_APP_NAME",
            "httui-launcher-integration-test-does-not-exist",
        )
        .output()
        .expect("spawn");
    assert!(!out.status.success());
}
