//! Pre-flight evaluator.
//!
//! Pure logic — `EvaluationContext` carries the in-memory snapshots
//! the consumer collected (active env vars, branch, connection
//! aliases). The evaluator returns one `CheckResult` per item.
//!
//! `FileExists` and `Command` checks need filesystem / process
//! access; the pure evaluator returns `Skip { reason: "needs FS/proc
//! evaluation" }` for those — a thin consumer-side wrapper layers
//! the syscalls on top once the UI mount lands. Keeping them
//! out of the pure layer keeps the evaluator deterministic and
//! easy to test.
//!
//! `Unknown` items (forward-compat fallback from the parser) always
//! produce `Skip { reason: "unknown check kind" }`.

use std::collections::HashSet;

use serde::Serialize;

use super::PreflightItem;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum CheckResult {
    Pass,
    Fail { reason: String },
    Skip { reason: String },
}

/// Snapshot of the runtime state needed for evaluation. Each field
/// is set by the consumer to the live state of the vault/session;
/// missing data passes `None` so the evaluator can `Skip` cleanly.
pub struct EvaluationContext<'a> {
    /// Current git branch. `None` when the vault is not a git repo —
    /// `Branch` checks `Skip` in that case.
    pub branch: Option<&'a str>,
    /// Active environment's variable keys. Used for `EnvVar` checks.
    pub active_env_vars: &'a HashSet<String>,
    /// Connection aliases configured for this vault.
    pub connections: &'a HashSet<String>,
}

pub fn evaluate_preflight(
    items: &[PreflightItem],
    ctx: &EvaluationContext<'_>,
) -> Vec<CheckResult> {
    items.iter().map(|item| evaluate_one(item, ctx)).collect()
}

pub(crate) fn evaluate_one(item: &PreflightItem, ctx: &EvaluationContext<'_>) -> CheckResult {
    match item {
        PreflightItem::Connection { name } => {
            if ctx.connections.contains(name) {
                CheckResult::Pass
            } else {
                CheckResult::Fail {
                    reason: format!("connection `{name}` not found"),
                }
            }
        }
        PreflightItem::EnvVar { name } => {
            if ctx.active_env_vars.contains(name) {
                CheckResult::Pass
            } else {
                CheckResult::Fail {
                    reason: format!("env var `{name}` not set in active environment"),
                }
            }
        }
        PreflightItem::Branch { name } => match ctx.branch {
            None => CheckResult::Skip {
                reason: "vault is not a git repository".into(),
            },
            Some(current) if current == name => CheckResult::Pass,
            Some(current) => CheckResult::Fail {
                reason: format!("on branch `{current}`, expected `{name}`"),
            },
        },
        PreflightItem::FileExists { .. } => CheckResult::Skip {
            reason: "needs FS evaluation".into(),
        },
        PreflightItem::Command { .. } => CheckResult::Skip {
            reason: "needs process evaluation".into(),
        },
        PreflightItem::Unknown { key, .. } => CheckResult::Skip {
            reason: format!("unknown check kind `{key}`"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn ctx_with(
        branch: Option<&'static str>,
        envs: &[&str],
        conns: &[&str],
    ) -> (HashSet<String>, HashSet<String>, Option<&'static str>) {
        (
            envs.iter().map(|s| s.to_string()).collect(),
            conns.iter().map(|s| s.to_string()).collect(),
            branch,
        )
    }

    fn make_ctx<'a>(
        branch: &'a Option<&'static str>,
        envs: &'a HashSet<String>,
        conns: &'a HashSet<String>,
    ) -> EvaluationContext<'a> {
        EvaluationContext {
            branch: *branch,
            active_env_vars: envs,
            connections: conns,
        }
    }

    #[test]
    fn connection_pass_when_alias_present() {
        let (envs, conns, branch) = ctx_with(None, &[], &["payments-db"]);
        let ctx = make_ctx(&branch, &envs, &conns);
        let r = evaluate_preflight(
            &[PreflightItem::Connection {
                name: "payments-db".into(),
            }],
            &ctx,
        );
        assert_eq!(r, vec![CheckResult::Pass]);
    }

    #[test]
    fn connection_fail_when_alias_missing() {
        let (envs, conns, branch) = ctx_with(None, &[], &["other"]);
        let ctx = make_ctx(&branch, &envs, &conns);
        let r = evaluate_preflight(
            &[PreflightItem::Connection {
                name: "payments-db".into(),
            }],
            &ctx,
        );
        assert!(matches!(r[0], CheckResult::Fail { .. }));
        if let CheckResult::Fail { reason } = &r[0] {
            assert!(reason.contains("payments-db"));
        }
    }

    #[test]
    fn env_var_pass_and_fail() {
        let (envs, conns, branch) = ctx_with(None, &["API_TOKEN"], &[]);
        let ctx = make_ctx(&branch, &envs, &conns);
        let r = evaluate_preflight(
            &[
                PreflightItem::EnvVar {
                    name: "API_TOKEN".into(),
                },
                PreflightItem::EnvVar {
                    name: "MISSING".into(),
                },
            ],
            &ctx,
        );
        assert_eq!(r[0], CheckResult::Pass);
        assert!(matches!(r[1], CheckResult::Fail { .. }));
    }

    #[test]
    fn branch_skip_when_not_a_repo() {
        let (envs, conns, branch) = ctx_with(None, &[], &[]);
        let ctx = make_ctx(&branch, &envs, &conns);
        let r = evaluate_preflight(
            &[PreflightItem::Branch {
                name: "main".into(),
            }],
            &ctx,
        );
        assert!(matches!(r[0], CheckResult::Skip { .. }));
    }

    #[test]
    fn branch_pass_when_matches() {
        let (envs, conns, branch) = ctx_with(Some("main"), &[], &[]);
        let ctx = make_ctx(&branch, &envs, &conns);
        let r = evaluate_preflight(
            &[PreflightItem::Branch {
                name: "main".into(),
            }],
            &ctx,
        );
        assert_eq!(r[0], CheckResult::Pass);
    }

    #[test]
    fn branch_fail_with_current_branch_in_reason() {
        let (envs, conns, branch) = ctx_with(Some("feat/x"), &[], &[]);
        let ctx = make_ctx(&branch, &envs, &conns);
        let r = evaluate_preflight(
            &[PreflightItem::Branch {
                name: "main".into(),
            }],
            &ctx,
        );
        if let CheckResult::Fail { reason } = &r[0] {
            assert!(reason.contains("feat/x"));
            assert!(reason.contains("main"));
        } else {
            panic!("expected Fail, got {:?}", r[0]);
        }
    }

    #[test]
    fn file_exists_skips_in_pure_layer() {
        let (envs, conns, branch) = ctx_with(None, &[], &[]);
        let ctx = make_ctx(&branch, &envs, &conns);
        let r = evaluate_preflight(&[PreflightItem::FileExists { path: "x".into() }], &ctx);
        if let CheckResult::Skip { reason } = &r[0] {
            assert!(reason.contains("FS"));
        } else {
            panic!("expected Skip");
        }
    }

    #[test]
    fn command_skips_in_pure_layer() {
        let (envs, conns, branch) = ctx_with(None, &[], &[]);
        let ctx = make_ctx(&branch, &envs, &conns);
        let r = evaluate_preflight(
            &[PreflightItem::Command {
                command: "psql --version".into(),
            }],
            &ctx,
        );
        if let CheckResult::Skip { reason } = &r[0] {
            assert!(reason.contains("process"));
        } else {
            panic!("expected Skip");
        }
    }

    #[test]
    fn unknown_kind_skips_with_key_in_reason() {
        let (envs, conns, branch) = ctx_with(None, &[], &[]);
        let ctx = make_ctx(&branch, &envs, &conns);
        let r = evaluate_preflight(
            &[PreflightItem::Unknown {
                key: "future_kind".into(),
                value: "x".into(),
            }],
            &ctx,
        );
        if let CheckResult::Skip { reason } = &r[0] {
            assert!(reason.contains("future_kind"));
        } else {
            panic!("expected Skip");
        }
    }

    #[test]
    fn evaluate_preflight_preserves_input_order() {
        let (envs, conns, branch) = ctx_with(Some("main"), &["A"], &["c1"]);
        let ctx = make_ctx(&branch, &envs, &conns);
        let items = vec![
            PreflightItem::Connection { name: "c1".into() },
            PreflightItem::EnvVar { name: "A".into() },
            PreflightItem::Branch {
                name: "main".into(),
            },
        ];
        let r = evaluate_preflight(&items, &ctx);
        assert_eq!(
            r,
            vec![CheckResult::Pass, CheckResult::Pass, CheckResult::Pass]
        );
    }

    #[test]
    fn check_result_serializes_with_outcome_tag() {
        let pass = serde_json::to_string(&CheckResult::Pass).unwrap();
        assert!(pass.contains("\"outcome\":\"pass\""));
        let fail = serde_json::to_string(&CheckResult::Fail { reason: "x".into() }).unwrap();
        assert!(fail.contains("\"outcome\":\"fail\""));
        assert!(fail.contains("\"reason\":\"x\""));
    }
}
