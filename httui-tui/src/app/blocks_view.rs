//! BLOCKS view state — fullscreen single-block alternative to the
//! markdown editor view.

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlocksViewKind {
    Http,
}

impl BlocksViewKind {
    pub fn region_count(self) -> usize {
        match self {
            BlocksViewKind::Http => 4,
        }
    }

    pub fn region_label(self, index: usize) -> &'static str {
        match (self, index) {
            (BlocksViewKind::Http, 0) => "Request",
            (BlocksViewKind::Http, 1) => "Headers",
            (BlocksViewKind::Http, 2) => "Body",
            (BlocksViewKind::Http, 3) => "Response",
            _ => "?",
        }
    }
}

#[derive(Debug)]
pub struct BlocksViewState {
    #[allow(dead_code)]
    pub file_path: PathBuf,
    pub segment_idx: usize,
    pub kind: BlocksViewKind,
    pub region: usize,
}

impl BlocksViewState {
    pub fn new(file_path: PathBuf, segment_idx: usize, kind: BlocksViewKind) -> Self {
        Self {
            file_path,
            segment_idx,
            kind,
            region: 0,
        }
    }

    pub fn next_region(&mut self) {
        let count = self.kind.region_count();
        if count == 0 {
            return;
        }
        self.region = (self.region + 1) % count;
    }

    pub fn prev_region(&mut self) {
        let count = self.kind.region_count();
        if count == 0 {
            return;
        }
        self.region = (self.region + count - 1) % count;
    }

    pub fn set_region(&mut self, index: usize) {
        let count = self.kind.region_count();
        if count == 0 {
            return;
        }
        self.region = index.min(count - 1);
    }

    pub fn current_region_label(&self) -> &'static str {
        self.kind.region_label(self.region)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn http_state() -> BlocksViewState {
        BlocksViewState::new(PathBuf::from("api.md"), 0, BlocksViewKind::Http)
    }

    #[test]
    fn http_kind_has_four_regions() {
        assert_eq!(BlocksViewKind::Http.region_count(), 4);
        for i in 0..4 {
            assert_ne!(BlocksViewKind::Http.region_label(i), "?");
        }
        assert_eq!(BlocksViewKind::Http.region_label(4), "?");
    }

    #[test]
    fn new_starts_focused_on_region_zero() {
        let s = http_state();
        assert_eq!(s.region, 0);
        assert_eq!(s.current_region_label(), "Request");
    }

    #[test]
    fn next_region_cycles_forward_and_wraps() {
        let mut s = http_state();
        s.next_region();
        assert_eq!(s.region, 1);
        s.next_region();
        assert_eq!(s.region, 2);
        s.next_region();
        assert_eq!(s.region, 3);
        s.next_region();
        assert_eq!(s.region, 0);
    }

    #[test]
    fn prev_region_cycles_backward_and_wraps() {
        let mut s = http_state();
        s.prev_region();
        assert_eq!(s.region, 3);
        s.prev_region();
        assert_eq!(s.region, 2);
    }

    #[test]
    fn set_region_clamps_to_last() {
        let mut s = http_state();
        s.set_region(2);
        assert_eq!(s.region, 2);
        s.set_region(99);
        assert_eq!(s.region, 3);
    }

    #[test]
    fn current_region_label_tracks_focus() {
        let mut s = http_state();
        assert_eq!(s.current_region_label(), "Request");
        s.set_region(1);
        assert_eq!(s.current_region_label(), "Headers");
        s.set_region(2);
        assert_eq!(s.current_region_label(), "Body");
        s.set_region(3);
        assert_eq!(s.current_region_label(), "Response");
    }
}
