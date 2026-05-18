//! IO-aware pre-flight evaluator.
//!
//! The pure evaluator returns `Skip { reason: "needs FS/proc evaluation" }`
//! for `FileExists` and `Command` because those need filesystem and
//! process access — keeping them out of the pure layer keeps it
//! deterministic. This module is the consumer-side wrapper that
//! resolves both kinds against the host:
//!
//! - `FileExists { path }` — relative paths are joined to `vault_root`,
//!   absolute paths used as-is. `Pass` when `metadata()` succeeds for
//!   any kind of entry (file or dir); `Fail` otherwise.
//! - `Command { command }` — the *first* whitespace-delimited token of
//!   `command` is the executable name. We walk `PATH`, append the
//!   exe name (with `.exe` suffix on Windows), and `Pass` on the
//!   first hit. We never *run* the command — presence in `PATH` is
//!   the contract per canvas spec.
//!
//! The wrapper preserves input order and falls through to
//! [`evaluate_one`] for every other variant, so it is a strict
//! superset of [`evaluate_preflight`].

use std::path::{Path, PathBuf};

use super::evaluator::evaluate_one;
use super::{CheckResult, EvaluationContext, PreflightItem};

/// Evaluate every item, layering FS + process resolution on top of
/// the pure logic. `vault_root` is the absolute path of the open
/// vault — relative `FileExists` paths resolve against it.
pub fn evaluate_preflight_with_io(
    items: &[PreflightItem],
    ctx: &EvaluationContext<'_>,
    vault_root: &Path,
) -> Vec<CheckResult> {
    items
        .iter()
        .map(|item| match item {
            PreflightItem::FileExists { path } => evaluate_file_exists(path, vault_root),
            PreflightItem::Command { command } => evaluate_command(command),
            other => evaluate_one(other, ctx),
        })
        .collect()
}

fn evaluate_file_exists(path: &str, vault_root: &Path) -> CheckResult {
    if path.trim().is_empty() {
        return CheckResult::Fail {
            reason: "empty file path".into(),
        };
    }
    let candidate = PathBuf::from(path);
    let resolved = if candidate.is_absolute() {
        candidate
    } else {
        vault_root.join(&candidate)
    };
    match std::fs::metadata(&resolved) {
        Ok(_) => CheckResult::Pass,
        Err(err) => CheckResult::Fail {
            reason: format!("file `{}` not found ({err})", resolved.display()),
        },
    }
}

fn evaluate_command(command: &str) -> CheckResult {
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    evaluate_command_with_path(command, &path_var)
}

/// PATH-injectable variant — the production path goes through
/// [`evaluate_command`] which reads `$PATH` once. This split exists
/// so tests can pass a fabricated PATH without mutating the
/// process-global env (which would race under `cargo test`'s
/// parallel runner).
fn evaluate_command_with_path(command: &str, path_var: &std::ffi::OsStr) -> CheckResult {
    let exe = match command.split_whitespace().next() {
        Some(token) if !token.is_empty() => token,
        _ => {
            return CheckResult::Fail {
                reason: "empty command string".into(),
            }
        }
    };
    if exe.contains('/') || exe.contains('\\') {
        // Path-qualified command — check the literal binary, do not
        // walk PATH. Mirrors how a shell resolves `./bin/x` or
        // `/usr/local/bin/psql`.
        return match std::fs::metadata(exe) {
            Ok(_) => CheckResult::Pass,
            Err(_) => CheckResult::Fail {
                reason: format!("command `{exe}` not found on disk"),
            },
        };
    }
    if which_in_path(exe, path_var) {
        CheckResult::Pass
    } else {
        CheckResult::Fail {
            reason: format!("command `{exe}` not found in PATH"),
        }
    }
}

fn which_in_path(exe: &str, path_var: &std::ffi::OsStr) -> bool {
    for dir in std::env::split_paths(path_var) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        if cfg!(windows) {
            for ext in windows_exe_exts() {
                let mut candidate = dir.join(exe);
                if !ext.is_empty() {
                    let mut name = candidate.file_name().unwrap_or_default().to_os_string();
                    name.push(ext);
                    candidate.set_file_name(name);
                }
                if candidate.is_file() {
                    return true;
                }
            }
        } else {
            let candidate = dir.join(exe);
            if candidate.is_file() {
                return true;
            }
        }
    }
    false
}

#[cfg(windows)]
fn windows_exe_exts() -> Vec<&'static str> {
    vec!["", ".exe", ".cmd", ".bat", ".com"]
}

#[cfg(not(windows))]
fn windows_exe_exts() -> Vec<&'static str> {
    vec![""]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::fs;
    use tempfile::tempdir;

    fn empty_ctx<'a>(
        envs: &'a HashSet<String>,
        conns: &'a HashSet<String>,
    ) -> EvaluationContext<'a> {
        EvaluationContext {
            branch: None,
            active_env_vars: envs,
            connections: conns,
        }
    }

    #[test]
    fn file_exists_resolves_relative_against_vault_root() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("schema.sql");
        fs::write(&target, "").unwrap();
        let envs = HashSet::new();
        let conns = HashSet::new();
        let ctx = empty_ctx(&envs, &conns);
        let r = evaluate_preflight_with_io(
            &[PreflightItem::FileExists {
                path: "schema.sql".into(),
            }],
            &ctx,
            dir.path(),
        );
        assert_eq!(r[0], CheckResult::Pass);
    }

    #[test]
    fn file_exists_passes_for_directory() {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("schema");
        fs::create_dir(&sub).unwrap();
        let envs = HashSet::new();
        let conns = HashSet::new();
        let ctx = empty_ctx(&envs, &conns);
        let r = evaluate_preflight_with_io(
            &[PreflightItem::FileExists {
                path: "schema".into(),
            }],
            &ctx,
            dir.path(),
        );
        assert_eq!(r[0], CheckResult::Pass);
    }

    #[test]
    fn file_exists_fails_when_missing() {
        let dir = tempdir().unwrap();
        let envs = HashSet::new();
        let conns = HashSet::new();
        let ctx = empty_ctx(&envs, &conns);
        let r = evaluate_preflight_with_io(
            &[PreflightItem::FileExists {
                path: "nope.sql".into(),
            }],
            &ctx,
            dir.path(),
        );
        if let CheckResult::Fail { reason } = &r[0] {
            assert!(reason.contains("nope.sql"));
        } else {
            panic!("expected Fail, got {:?}", r[0]);
        }
    }

    #[test]
    fn file_exists_handles_absolute_path_directly() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("absolute.sql");
        fs::write(&target, "").unwrap();
        let envs = HashSet::new();
        let conns = HashSet::new();
        let ctx = empty_ctx(&envs, &conns);
        // vault_root is unrelated; absolute path should resolve.
        let other_root = tempdir().unwrap();
        let r = evaluate_preflight_with_io(
            &[PreflightItem::FileExists {
                path: target.to_string_lossy().into_owned(),
            }],
            &ctx,
            other_root.path(),
        );
        assert_eq!(r[0], CheckResult::Pass);
    }

    #[test]
    fn file_exists_fails_with_empty_path() {
        let dir = tempdir().unwrap();
        let envs = HashSet::new();
        let conns = HashSet::new();
        let ctx = empty_ctx(&envs, &conns);
        let r = evaluate_preflight_with_io(
            &[PreflightItem::FileExists { path: "   ".into() }],
            &ctx,
            dir.path(),
        );
        assert!(matches!(r[0], CheckResult::Fail { .. }));
    }

    #[test]
    fn which_in_path_finds_executable_in_supplied_path() {
        // No env mutation — pass a fabricated PATH directly so the
        // test stays parallel-safe under cargo test / cargo llvm-cov.
        let dir = tempdir().unwrap();
        let exe_name = if cfg!(windows) {
            "myfaketool.exe"
        } else {
            "myfaketool"
        };
        let exe_path = dir.path().join(exe_name);
        fs::write(&exe_path, b"#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&exe_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&exe_path, perms).unwrap();
        }
        let path_var = dir.path().as_os_str();
        let r = evaluate_command_with_path("myfaketool --help", path_var);
        assert_eq!(r, CheckResult::Pass);
    }

    #[test]
    fn which_in_path_fails_when_executable_not_under_supplied_path() {
        let dir = tempdir().unwrap(); // empty
        let path_var = dir.path().as_os_str();
        let r = evaluate_command_with_path("definitely-not-installed-xyz", path_var);
        if let CheckResult::Fail { reason } = &r {
            assert!(reason.contains("definitely-not-installed-xyz"));
            assert!(reason.contains("PATH"));
        } else {
            panic!("expected Fail, got {r:?}");
        }
    }

    #[test]
    fn which_in_path_skips_empty_segments() {
        let dir = tempdir().unwrap();
        let exe_name = if cfg!(windows) { "tool.exe" } else { "tool" };
        let exe = dir.path().join(exe_name);
        fs::write(&exe, b"").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&exe).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&exe, perms).unwrap();
        }
        // Insert empty path segments around the real one — they
        // should be skipped, not treated as cwd-relative.
        let sep = if cfg!(windows) { ";" } else { ":" };
        let mut path_str = String::new();
        path_str.push_str(sep);
        path_str.push_str(&dir.path().to_string_lossy());
        path_str.push_str(sep);
        let r = evaluate_command_with_path("tool --version", std::ffi::OsStr::new(&path_str));
        assert_eq!(r, CheckResult::Pass);
    }

    #[test]
    fn command_fails_with_empty_string() {
        let dir = tempdir().unwrap();
        let envs = HashSet::new();
        let conns = HashSet::new();
        let ctx = empty_ctx(&envs, &conns);
        let r = evaluate_preflight_with_io(
            &[PreflightItem::Command {
                command: "   ".into(),
            }],
            &ctx,
            dir.path(),
        );
        assert!(matches!(r[0], CheckResult::Fail { .. }));
    }

    #[test]
    fn command_path_qualified_checks_literal_binary() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("tool");
        fs::write(&target, "").unwrap();
        let envs = HashSet::new();
        let conns = HashSet::new();
        let ctx = empty_ctx(&envs, &conns);
        let r = evaluate_preflight_with_io(
            &[PreflightItem::Command {
                command: format!("{} --version", target.display()),
            }],
            &ctx,
            dir.path(),
        );
        assert_eq!(r[0], CheckResult::Pass);

        let r2 = evaluate_preflight_with_io(
            &[PreflightItem::Command {
                command: format!("{} --version", dir.path().join("missing").display()),
            }],
            &ctx,
            dir.path(),
        );
        if let CheckResult::Fail { reason } = &r2[0] {
            assert!(reason.contains("missing"));
        } else {
            panic!("expected Fail, got {:?}", r2[0]);
        }
    }

    #[test]
    fn delegates_other_kinds_to_pure_evaluator() {
        let dir = tempdir().unwrap();
        let envs: HashSet<String> = ["API_TOKEN".into()].iter().cloned().collect();
        let conns: HashSet<String> = ["payments-db".into()].iter().cloned().collect();
        let ctx = EvaluationContext {
            branch: Some("main"),
            active_env_vars: &envs,
            connections: &conns,
        };
        let items = vec![
            PreflightItem::Connection {
                name: "payments-db".into(),
            },
            PreflightItem::EnvVar {
                name: "API_TOKEN".into(),
            },
            PreflightItem::Branch {
                name: "main".into(),
            },
            PreflightItem::Unknown {
                key: "future".into(),
                value: "x".into(),
            },
        ];
        let r = evaluate_preflight_with_io(&items, &ctx, dir.path());
        assert_eq!(r[0], CheckResult::Pass);
        assert_eq!(r[1], CheckResult::Pass);
        assert_eq!(r[2], CheckResult::Pass);
        assert!(matches!(r[3], CheckResult::Skip { .. })); // unknown kind
    }

    #[test]
    fn preserves_input_order_across_mixed_kinds() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("present.sql");
        fs::write(&target, "").unwrap();
        let envs = HashSet::new();
        let conns = HashSet::new();
        let ctx = empty_ctx(&envs, &conns);
        let r = evaluate_preflight_with_io(
            &[
                PreflightItem::FileExists {
                    path: "present.sql".into(),
                },
                PreflightItem::FileExists {
                    path: "absent.sql".into(),
                },
            ],
            &ctx,
            dir.path(),
        );
        assert_eq!(r[0], CheckResult::Pass);
        assert!(matches!(r[1], CheckResult::Fail { .. }));
    }
}
