use ratatui::{layout::Rect, Frame};

use crate::app::App;

mod pane;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    pane::render(frame, area, app);
}
