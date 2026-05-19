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
}

impl ResultPanelTab {
    pub fn label(self) -> &'static str {
        match self {
            ResultPanelTab::Result => "Result",
            ResultPanelTab::Messages => "Messages",
            ResultPanelTab::Plan => "Plan",
            ResultPanelTab::Stats => "Stats",
        }
    }

    /// Block-type-aware label. HTTP repurposes the 4 slots as
    /// Body / Headers / Cookies / Stats so the tab strip reads
    /// like the desktop's response viewer. DB and unknown types
    /// fall through to the default `label()`.
    pub fn label_for(self, block_type: &str) -> &'static str {
        if block_type == "http" {
            return match self {
                ResultPanelTab::Result => "Body",
                ResultPanelTab::Messages => "Headers",
                ResultPanelTab::Plan => "Cookies",
                ResultPanelTab::Stats => "Stats",
            };
        }
        self.label()
    }

    pub fn next(self) -> Self {
        match self {
            ResultPanelTab::Result => ResultPanelTab::Messages,
            ResultPanelTab::Messages => ResultPanelTab::Plan,
            ResultPanelTab::Plan => ResultPanelTab::Stats,
            ResultPanelTab::Stats => ResultPanelTab::Result,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            ResultPanelTab::Result => ResultPanelTab::Stats,
            ResultPanelTab::Messages => ResultPanelTab::Result,
            ResultPanelTab::Plan => ResultPanelTab::Messages,
            ResultPanelTab::Stats => ResultPanelTab::Plan,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ResultPanelTab;

    #[test]
    fn tab_next_cycles_forward_with_wrap() {
        // Result → Messages → Plan → Stats → Result. The wrap is
        // important: `gt` keeps spinning instead of getting stuck
        // at the end.
        let mut t = ResultPanelTab::default();
        assert_eq!(t, ResultPanelTab::Result);
        t = t.next();
        assert_eq!(t, ResultPanelTab::Messages);
        t = t.next();
        assert_eq!(t, ResultPanelTab::Plan);
        t = t.next();
        assert_eq!(t, ResultPanelTab::Stats);
        t = t.next();
        assert_eq!(t, ResultPanelTab::Result);
    }

    #[test]
    fn tab_prev_inverts_next() {
        // Walking back is the mirror of walking forward — useful
        // when the user overshoots with `gt` and needs `gT` to
        // back out.
        let mut t = ResultPanelTab::default();
        for _ in 0..4 {
            let forward = t.next();
            let back = forward.prev();
            assert_eq!(back, t);
            t = forward;
        }
    }

    #[test]
    fn label_returns_db_oriented_names() {
        // The default (DB) labels read like a query-result viewer.
        assert_eq!(ResultPanelTab::Result.label(), "Result");
        assert_eq!(ResultPanelTab::Messages.label(), "Messages");
        assert_eq!(ResultPanelTab::Plan.label(), "Plan");
        assert_eq!(ResultPanelTab::Stats.label(), "Stats");
    }

    #[test]
    fn label_for_http_repurposes_the_four_slots() {
        // HTTP reads like the desktop's response viewer: the same
        // four slots become Body / Headers / Cookies / Stats.
        assert_eq!(ResultPanelTab::Result.label_for("http"), "Body");
        assert_eq!(ResultPanelTab::Messages.label_for("http"), "Headers");
        assert_eq!(ResultPanelTab::Plan.label_for("http"), "Cookies");
        assert_eq!(ResultPanelTab::Stats.label_for("http"), "Stats");
    }

    #[test]
    fn label_for_non_http_falls_back_to_default_label() {
        // DB and unknown block types fall through to `label()` —
        // verify both a known (`db-sqlite`) and an arbitrary type.
        for bt in ["db-sqlite", "db-postgres", "unknown", ""] {
            assert_eq!(ResultPanelTab::Result.label_for(bt), "Result");
            assert_eq!(ResultPanelTab::Messages.label_for(bt), "Messages");
            assert_eq!(ResultPanelTab::Plan.label_for(bt), "Plan");
            assert_eq!(ResultPanelTab::Stats.label_for(bt), "Stats");
        }
    }
}
