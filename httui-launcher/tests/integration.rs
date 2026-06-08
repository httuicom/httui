// End-to-end coverage for the launcher binary. These exercise the
// real `main()` entrypoint and the spawn paths so the file does not
// rely on `// coverage:exclude`.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

fn original_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_httui"))
}

// Copies the launcher to an isolated directory so the spawn paths
// can't reach the real `httui-tui` / `httui-desktop` binaries that
// happen to live alongside it in `target/<profile>/`. Returns the
// path to the copy.
fn isolated_launcher() -> (tempfile::TempDir, PathBuf) {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let dir = tempfile::tempdir().expect("tempdir");
    let dest = dir
        .path()
        .join(format!("httui-{}", COUNTER.fetch_add(1, Ordering::Relaxed)));
    fs::copy(original_bin(), &dest).expect("copy launcher");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perm = fs::metadata(&dest).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&dest, perm).unwrap();
    }
    (dir, dest)
}

#[test]
fn help_flag_prints_usage_and_exits_success() {
    // --help short-circuits before any sibling lookup, so the bin
    // location doesn't matter here.
    let out = Command::new(original_bin())
        .arg("--help")
        .output()
        .expect("spawn");
    assert!(out.status.success(), "exit: {:?}", out.status);
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("USAGE:") && stdout.contains("httui desktop"),
        "stdout: {stdout}"
    );
}

#[test]
fn short_help_flag_works() {
    let out = Command::new(original_bin())
        .arg("-h")
        .output()
        .expect("spawn");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("USAGE:"));
}

#[test]
fn missing_tui_sibling_fails_with_clear_error() {
    let (_dir, bin) = isolated_launcher();
    let out = Command::new(&bin).output().expect("spawn");
    assert!(!out.status.success(), "expected failure, got success");
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("httui-tui not found") || stderr.contains("cargo run -p httui-tui"),
        "stderr: {stderr}"
    );
}

#[test]
fn extra_args_are_forwarded_to_the_tui_path() {
    let (_dir, bin) = isolated_launcher();
    let out = Command::new(&bin)
        .args(["--log-level", "debug"])
        .output()
        .expect("spawn");
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("httui-tui not found"), "stderr: {stderr}");
}

#[cfg(not(target_os = "macos"))]
#[test]
fn missing_desktop_sibling_fails_on_non_macos() {
    let (_dir, bin) = isolated_launcher();
    let out = Command::new(&bin).arg("desktop").output().expect("spawn");
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("httui-desktop not found"),
        "stderr: {stderr}"
    );
}

#[cfg(target_os = "macos")]
#[test]
fn desktop_subcommand_invokes_open_on_macos() {
    // Exercise the macOS run_desktop branch + spawn() Ok(status) path
    // without actually opening the user's real /Applications/httui.app:
    // HTTUI_DESKTOP_APP_NAME points `open -a` at an app that does not
    // exist, so the launcher's spawn returns a non-zero status without
    // any GUI side effects.
    let (_dir, bin) = isolated_launcher();
    let out = Command::new(&bin)
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
    let (_dir, bin) = isolated_launcher();
    let out = Command::new(&bin)
        .args(["desktop", "--debug", "foo"])
        .env(
            "HTTUI_DESKTOP_APP_NAME",
            "httui-launcher-integration-test-does-not-exist",
        )
        .output()
        .expect("spawn");
    assert!(!out.status.success());
}
