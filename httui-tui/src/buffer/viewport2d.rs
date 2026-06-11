//! Horizontal viewport math shared by every text surface.
//!
//! Cursor offsets across the app are CHAR columns (rope-native); the
//! terminal paints DISPLAY columns. [`display_col`] is the single
//! boundary between the two — everything else in this module speaks
//! display columns only. Width is summed per-char via `unicode-width`
//! with `width(control) = 0`, which mirrors ratatui's painter (it
//! skips zero-width symbols, so tabs occupy no cell). Multi-char
//! grapheme clusters (ZWJ emoji) may drift a cell relative to some
//! terminals, but they drift identically in the painter, so cursor
//! and text stay in lock-step.

use unicode_width::UnicodeWidthChar;

pub(crate) const H_SCROLL_OFF: u16 = 3;

/// Display column of `char_col` within a line. The ONE char→display
/// conversion point.
pub(crate) fn display_col(chars: impl Iterator<Item = char>, char_col: usize) -> u16 {
    chars
        .take(char_col)
        .map(|c| c.width().unwrap_or(0) as u16)
        .fold(0u16, u16::saturating_add)
}

/// Adjust `left` so `cursor_x` stays inside
/// `[left + scrolloff, left + width - scrolloff)`. Returns the new
/// left. X-axis mirror of `clamp_viewport`; stateless callers pass
/// `left = 0` and get keep-visible-with-right-margin behavior.
pub(crate) fn follow_x(left: u16, width: u16, cursor_x: u16) -> u16 {
    if width == 0 {
        return left;
    }
    let scrolloff = H_SCROLL_OFF.min(width / 2);
    let upper = cursor_x.saturating_sub(scrolloff);
    let lower = cursor_x.saturating_add(scrolloff + 1).saturating_sub(width);
    if left > upper {
        upper
    } else if left < lower {
        lower
    } else {
        left
    }
}

/// Screen X of display column `cursor_x` under a pan of `left`.
/// `None` means the cursor falls outside the visible window and must
/// be hidden (not clamped — a clamped caret lies about its column).
pub(crate) fn project_x(cursor_x: u16, left: u16, area_x: u16, area_width: u16) -> Option<u16> {
    if cursor_x < left {
        return None;
    }
    let rel = cursor_x - left;
    if rel >= area_width {
        return None;
    }
    Some(area_x.saturating_add(rel))
}

/// Slice `text` to the display-column window `[left, left + width)`.
/// Byte-safe; a wide char straddling either edge is dropped rather
/// than split (same trade-off as ratatui's line truncation). For
/// surfaces where `Paragraph::scroll` does not apply — it is ignored
/// when `Wrap` is set, and span-level slicing is needed when only one
/// span of a line should pan.
pub(crate) fn window_slice(text: &str, left: u16, width: u16) -> &str {
    if width == 0 {
        return "";
    }
    let left = left as usize;
    let right = left + width as usize;
    let mut col = 0usize;
    let mut start_byte: Option<usize> = None;
    let mut end_byte = text.len();
    for (i, c) in text.char_indices() {
        let next = col + c.width().unwrap_or(0);
        if start_byte.is_none() && col >= left {
            start_byte = Some(i);
        }
        if next > right {
            end_byte = i;
            break;
        }
        col = next;
    }
    match start_byte {
        Some(s) if s <= end_byte => &text[s..end_byte],
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follow_x_scrolls_right_to_keep_cursor_visible() {
        // scrolloff = 3 → lower = 100 + 4 - 40 = 64.
        let left = follow_x(0, 40, 100);
        assert_eq!(left, 64);
        assert!(100 - left < 40, "cursor must land inside the window");
    }

    #[test]
    fn follow_x_scrolls_left_when_cursor_before_window() {
        // upper = 20 - 3 = 17.
        assert_eq!(follow_x(50, 40, 20), 17);
    }

    #[test]
    fn follow_x_keeps_left_when_cursor_inside_window() {
        assert_eq!(follow_x(10, 40, 30), 10);
    }

    #[test]
    fn follow_x_returns_to_zero_at_line_start() {
        assert_eq!(follow_x(64, 40, 0), 0);
    }

    #[test]
    fn follow_x_zero_width_is_noop() {
        assert_eq!(follow_x(7, 0, 100), 7);
    }

    #[test]
    fn follow_x_caps_scrolloff_at_half_width() {
        // width 4 → scrolloff = 2 → lower = 10 + 3 - 4 = 9.
        assert_eq!(follow_x(0, 4, 10), 9);
    }

    #[test]
    fn display_col_is_identity_for_ascii() {
        assert_eq!(display_col("hello".chars(), 3), 3);
    }

    #[test]
    fn display_col_counts_wide_chars_as_two() {
        assert_eq!(display_col("日本語".chars(), 2), 4);
    }

    #[test]
    fn display_col_counts_tab_as_zero() {
        // The painter skips zero-width symbols; the math must match.
        assert_eq!(display_col("a\tb".chars(), 2), 1);
    }

    #[test]
    fn display_col_clamps_past_line_end() {
        assert_eq!(display_col("ab".chars(), 99), 2);
    }

    #[test]
    fn display_col_empty_line_is_zero() {
        assert_eq!(display_col("".chars(), 5), 0);
    }

    #[test]
    fn project_x_maps_visible_column() {
        assert_eq!(project_x(10, 5, 2, 40), Some(7));
    }

    #[test]
    fn project_x_hides_cursor_left_of_window() {
        assert_eq!(project_x(3, 5, 2, 40), None);
    }

    #[test]
    fn project_x_hides_cursor_right_of_window() {
        assert_eq!(project_x(45, 5, 2, 40), None);
    }

    #[test]
    fn project_x_window_edges() {
        assert_eq!(project_x(5, 5, 2, 40), Some(2));
        assert_eq!(project_x(44, 5, 2, 40), Some(41));
    }

    #[test]
    fn window_slice_takes_middle_window() {
        assert_eq!(window_slice("abcdefghij", 3, 4), "defg");
    }

    #[test]
    fn window_slice_zero_left_returns_head() {
        assert_eq!(window_slice("abc", 0, 10), "abc");
    }

    #[test]
    fn window_slice_drops_wide_char_straddling_edges() {
        // Cols: 日 0-2, 本 2-4, 語 4-6. Window [1, 5) clips both
        // neighbors, keeping only the fully-contained 本.
        assert_eq!(window_slice("日本語", 1, 4), "本");
    }

    #[test]
    fn window_slice_left_past_end_is_empty() {
        assert_eq!(window_slice("ab", 5, 3), "");
    }

    #[test]
    fn window_slice_zero_width_is_empty() {
        assert_eq!(window_slice("abcdef", 3, 0), "");
    }

    #[test]
    fn window_slice_window_covering_tail() {
        assert_eq!(window_slice("abcdef", 4, 10), "ef");
    }
}
