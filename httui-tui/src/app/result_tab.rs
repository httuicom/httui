//! DB/HTTP result-panel tab selection.
//!
//! Mechanically extracted from `app.rs` (tui-v2 vertical 1, fase 2
//! p1-result_tab) — pure code move, no behavior change. Re-exported
//! from `app/mod.rs` so `crate::app::ResultPanelTab` keeps resolving.

/// Selected tab in the DB result panel. Single global state — every
/// DB block uses the same selection so cycling on one block carries
/// over when you jump to another. Default `Result`.
///
/// Order matches the visual order of the tab bar; `next()` / `prev()`
/// wrap so cycling is keyboard-friendly.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ResultPanelTab {
    #[default]
    Result,
    Messages,
    Plan,
    Stats,
    /// HTTP-only fifth tab: the request/response as raw HTTP-message
    /// text (status line + headers + blank + body). DB blocks never
    /// land here — `next_for("db-*")` cycles only the first four.
    Raw,
}

impl ResultPanelTab {
    pub fn label(self) -> &'static str {
        match self {
            ResultPanelTab::Result => "Result",
            ResultPanelTab::Messages => "Messages",
            ResultPanelTab::Plan => "Plan",
            ResultPanelTab::Stats => "Stats",
            ResultPanelTab::Raw => "Raw",
        }
    }

    /// Block-type-aware label. HTTP repurposes the slots as
    /// Body / Headers / Cookies / Timing / Raw so the tab strip
    /// reads like the desktop's response viewer. DB and unknown
    /// types fall through to the default `label()`.
    pub fn label_for(self, block_type: &str) -> &'static str {
        if block_type == "http" {
            return match self {
                ResultPanelTab::Result => "Body",
                ResultPanelTab::Messages => "Headers",
                ResultPanelTab::Plan => "Cookies",
                ResultPanelTab::Stats => "Timing",
                ResultPanelTab::Raw => "Raw",
            };
        }
        self.label()
    }

    /// Tabs visible for `block_type`. HTTP exposes all 5; DB skips
    /// `Raw` (no raw-message view for SQL).
    pub fn variants_for(block_type: &str) -> &'static [ResultPanelTab] {
        if block_type == "http" {
            &[
                ResultPanelTab::Result,
                ResultPanelTab::Messages,
                ResultPanelTab::Plan,
                ResultPanelTab::Stats,
                ResultPanelTab::Raw,
            ]
        } else {
            &[
                ResultPanelTab::Result,
                ResultPanelTab::Messages,
                ResultPanelTab::Plan,
                ResultPanelTab::Stats,
            ]
        }
    }

    /// Cycle forward through the tabs visible for `block_type`.
    /// Wraps at the end. If `self` isn't in the visible set (DB
    /// landing on `Raw`, impossible in practice) we restart from the
    /// first visible tab so the cycle stays well-defined.
    pub fn next_for(self, block_type: &str) -> Self {
        let v = Self::variants_for(block_type);
        let idx = v.iter().position(|t| *t == self).unwrap_or(0);
        v[(idx + 1) % v.len()]
    }

    pub fn prev_for(self, block_type: &str) -> Self {
        let v = Self::variants_for(block_type);
        let idx = v.iter().position(|t| *t == self).unwrap_or(0);
        v[(idx + v.len() - 1) % v.len()]
    }
}

#[cfg(test)]
mod tests {
    use super::ResultPanelTab;

    #[test]
    fn tab_next_db_cycles_four_with_wrap() {
        let mut t = ResultPanelTab::default();
        assert_eq!(t, ResultPanelTab::Result);
        t = t.next_for("db-sqlite");
        assert_eq!(t, ResultPanelTab::Messages);
        t = t.next_for("db-sqlite");
        assert_eq!(t, ResultPanelTab::Plan);
        t = t.next_for("db-sqlite");
        assert_eq!(t, ResultPanelTab::Stats);
        t = t.next_for("db-sqlite");
        assert_eq!(t, ResultPanelTab::Result);
    }

    #[test]
    fn tab_next_http_cycles_five_with_wrap() {
        let mut t = ResultPanelTab::default();
        t = t.next_for("http");
        assert_eq!(t, ResultPanelTab::Messages);
        t = t.next_for("http");
        assert_eq!(t, ResultPanelTab::Plan);
        t = t.next_for("http");
        assert_eq!(t, ResultPanelTab::Stats);
        t = t.next_for("http");
        assert_eq!(t, ResultPanelTab::Raw);
        t = t.next_for("http");
        assert_eq!(t, ResultPanelTab::Result);
    }

    #[test]
    fn tab_prev_inverts_next_per_block_type() {
        for bt in ["db-sqlite", "http"] {
            let mut t = ResultPanelTab::default();
            for _ in 0..5 {
                let forward = t.next_for(bt);
                let back = forward.prev_for(bt);
                assert_eq!(back, t, "block_type={bt}");
                t = forward;
            }
        }
    }

    #[test]
    fn label_returns_db_oriented_names() {
        assert_eq!(ResultPanelTab::Result.label(), "Result");
        assert_eq!(ResultPanelTab::Messages.label(), "Messages");
        assert_eq!(ResultPanelTab::Plan.label(), "Plan");
        assert_eq!(ResultPanelTab::Stats.label(), "Stats");
        assert_eq!(ResultPanelTab::Raw.label(), "Raw");
    }

    #[test]
    fn label_for_http_uses_response_viewer_names() {
        assert_eq!(ResultPanelTab::Result.label_for("http"), "Body");
        assert_eq!(ResultPanelTab::Messages.label_for("http"), "Headers");
        assert_eq!(ResultPanelTab::Plan.label_for("http"), "Cookies");
        assert_eq!(ResultPanelTab::Stats.label_for("http"), "Timing");
        assert_eq!(ResultPanelTab::Raw.label_for("http"), "Raw");
    }

    #[test]
    fn label_for_non_http_falls_back_to_default_label() {
        for bt in ["db-sqlite", "db-postgres", "unknown", ""] {
            assert_eq!(ResultPanelTab::Result.label_for(bt), "Result");
            assert_eq!(ResultPanelTab::Messages.label_for(bt), "Messages");
            assert_eq!(ResultPanelTab::Plan.label_for(bt), "Plan");
            assert_eq!(ResultPanelTab::Stats.label_for(bt), "Stats");
        }
    }

    #[test]
    fn variants_for_db_excludes_raw() {
        let v = ResultPanelTab::variants_for("db-sqlite");
        assert_eq!(v.len(), 4);
        assert!(!v.contains(&ResultPanelTab::Raw));
    }

    #[test]
    fn variants_for_http_includes_raw_last() {
        let v = ResultPanelTab::variants_for("http");
        assert_eq!(v.len(), 5);
        assert_eq!(v.last(), Some(&ResultPanelTab::Raw));
    }
}
