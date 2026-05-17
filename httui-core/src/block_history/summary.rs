//! Per-file last-run summary — Epic 50 Story 03 backend slice.
//!
//! Powers the `<DocHeaderMetaStrip>` "Last run 14:32 · 12 blocks ·
//! 1 failed" chip. Pure aggregation over `HistoryEntry[]` returned
//! by `list_history_for_file` so the consumer just calls one query
//! + this helper without bespoke per-row aggregation in TS.
//!
//! "Last run" is a heuristic — `block_run_history` has no run-all
//! session id. We approximate it as "every entry whose `ran_at`
//! falls within `SESSION_WINDOW_SECS` of the most recent entry's
//! `ran_at`". Run-all dispatches all blocks in one go, so the
//! per-block timestamps cluster within a couple of seconds even on
//! a slow vault. Single-block manual runs naturally surface as a
//! "1 block" session.

use std::collections::HashSet;

use chrono::DateTime;
use serde::{Deserialize, Serialize};

use super::types::HistoryEntry;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LastRunSummary {
    /// ISO-8601 timestamp of the most recent run. `None` when the
    /// file has no recorded runs.
    pub ran_at: Option<String>,
    /// Distinct `block_alias`es that ran in the most recent session.
    pub block_count: i64,
    /// Distinct `block_alias`es whose outcome wasn't `"ok"`.
    pub failed_count: i64,
}

/// Window width for "same run-all session" — anything within this
/// many seconds of the most recent entry's `ran_at` counts as part
/// of the same session.
const SESSION_WINDOW_SECS: i64 = 5;

/// True when `outcome` reports a successful run. Mirrors the
/// status taxonomy used by `executor::http` / `executor::db` —
/// `"ok"` is the only successful state; everything else is some
/// kind of failure (`"error"`, `"cancelled"`, `"timeout"`, …).
fn is_ok_outcome(outcome: &str) -> bool {
    outcome == "ok"
}

/// Aggregate `entries` into a `LastRunSummary`. Empty input yields
/// the "no runs yet" shape.
pub fn summarize_last_run(entries: &[HistoryEntry]) -> LastRunSummary {
    if entries.is_empty() {
        return LastRunSummary {
            ran_at: None,
            block_count: 0,
            failed_count: 0,
        };
    }
    // Defensive max() — `list_history_for_file` already orders DESC,
    // but this helper can be fed any slice.
    let latest_str = entries
        .iter()
        .map(|e| e.ran_at.as_str())
        .max()
        .expect("entries non-empty");
    let latest_ts = DateTime::parse_from_rfc3339(latest_str).ok();

    let mut session_aliases: HashSet<&str> = HashSet::new();
    let mut failed_aliases: HashSet<&str> = HashSet::new();
    for entry in entries {
        // When timestamps are parseable, restrict the session window;
        // when they aren't, fall through and include the row so a
        // malformed entry doesn't silently drop the count to zero.
        if let (Some(latest_ts), Ok(entry_ts)) =
            (latest_ts, DateTime::parse_from_rfc3339(&entry.ran_at))
        {
            let diff = (latest_ts - entry_ts).num_seconds();
            if !(0..=SESSION_WINDOW_SECS).contains(&diff) {
                continue;
            }
        }
        session_aliases.insert(entry.block_alias.as_str());
        if !is_ok_outcome(&entry.outcome) {
            failed_aliases.insert(entry.block_alias.as_str());
        }
    }

    LastRunSummary {
        ran_at: Some(latest_str.to_string()),
        block_count: session_aliases.len() as i64,
        failed_count: failed_aliases.len() as i64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(alias: &str, outcome: &str, ran_at: &str) -> HistoryEntry {
        HistoryEntry {
            id: 0,
            file_path: "rb.md".into(),
            block_alias: alias.into(),
            method: "GET".into(),
            url_canonical: "/".into(),
            status: Some(200),
            request_size: None,
            response_size: None,
            elapsed_ms: Some(10),
            outcome: outcome.into(),
            ran_at: ran_at.into(),
            plan: None,
        }
    }

    #[test]
    fn empty_yields_no_runs_yet() {
        let s = summarize_last_run(&[]);
        assert_eq!(s.ran_at, None);
        assert_eq!(s.block_count, 0);
        assert_eq!(s.failed_count, 0);
    }

    #[test]
    fn single_block_session_counts_one() {
        let entries = vec![entry("login", "ok", "2026-04-30T14:32:01Z")];
        let s = summarize_last_run(&entries);
        assert_eq!(s.ran_at.as_deref(), Some("2026-04-30T14:32:01Z"));
        assert_eq!(s.block_count, 1);
        assert_eq!(s.failed_count, 0);
    }

    #[test]
    fn run_all_session_groups_within_5s() {
        let entries = vec![
            // Latest entry at the head — list_history_for_file ordering.
            entry("b3", "ok", "2026-04-30T14:32:03Z"),
            entry("b2", "ok", "2026-04-30T14:32:02Z"),
            entry("b1", "ok", "2026-04-30T14:32:00Z"),
            // An older standalone run — outside the 5s window.
            entry("b1", "ok", "2026-04-30T13:00:00Z"),
        ];
        let s = summarize_last_run(&entries);
        assert_eq!(s.ran_at.as_deref(), Some("2026-04-30T14:32:03Z"));
        assert_eq!(s.block_count, 3);
        assert_eq!(s.failed_count, 0);
    }

    #[test]
    fn failed_count_groups_by_alias_not_run() {
        // Same alias failing twice in the same session counts once.
        let entries = vec![
            entry("flaky", "error", "2026-04-30T14:32:03Z"),
            entry("flaky", "error", "2026-04-30T14:32:01Z"),
            entry("steady", "ok", "2026-04-30T14:32:02Z"),
        ];
        let s = summarize_last_run(&entries);
        assert_eq!(s.block_count, 2);
        assert_eq!(s.failed_count, 1);
    }

    #[test]
    fn non_ok_outcomes_count_as_failed() {
        for outcome in ["error", "cancelled", "timeout", "anything_else"] {
            let entries = vec![entry("b1", outcome, "2026-04-30T14:32:00Z")];
            let s = summarize_last_run(&entries);
            assert_eq!(s.failed_count, 1, "outcome `{outcome}` → failed");
        }
    }

    #[test]
    fn unparseable_ran_at_falls_through_into_session() {
        // No grouping happens when timestamps don't parse — better
        // to surface the row than silently drop everything.
        let entries = vec![
            entry("b1", "ok", "garbage"),
            entry("b2", "error", "garbage"),
        ];
        let s = summarize_last_run(&entries);
        assert_eq!(s.block_count, 2);
        assert_eq!(s.failed_count, 1);
    }

    #[test]
    fn max_ran_at_picked_when_input_unsorted() {
        // Defensive — the fn doesn't assume input is sorted DESC.
        let entries = vec![
            entry("a", "ok", "2026-04-30T13:00:00Z"),
            entry("b", "ok", "2026-04-30T14:00:00Z"),
            entry("c", "ok", "2026-04-30T13:30:00Z"),
        ];
        let s = summarize_last_run(&entries);
        assert_eq!(s.ran_at.as_deref(), Some("2026-04-30T14:00:00Z"));
        // Only `b` lands within 5s of the max.
        assert_eq!(s.block_count, 1);
    }
}
