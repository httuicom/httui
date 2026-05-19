//! Pure input vocabulary — motions, operators, text objects and the
//! line-edit action set. Mechanically moved out of `vim/parser.rs`
//! (tui-v2 vertical 1, fase 1 p1) with no logic change.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    Left,
    Right,
    Up,
    Down,
    LineStart,
    FirstNonBlank,
    LineEnd,
    WordForward,
    WordBackward,
    WordEnd,
    DocStart,
    DocEnd,
    GotoLine(usize),
    HalfPageDown,
    HalfPageUp,
    /// `f<c>` — jump to the next occurrence of `<c>` on the current line.
    /// Inclusive: `df<c>` deletes through `<c>`.
    FindForward(char),
    /// `F<c>` — jump to the previous occurrence on the current line.
    FindBackward(char),
    /// `t<c>` — jump to the position immediately before the next `<c>`.
    /// Inclusive: `dt<c>` deletes up to but not including `<c>`.
    TillForward(char),
    /// `T<c>` — jump to the position immediately after the previous `<c>`.
    TillBackward(char),
}

impl Motion {
    /// Reverse a find/till for `,` (repeat in opposite direction).
    /// Returns `None` for non-find motions.
    pub fn reverse_find(self) -> Option<Motion> {
        Some(match self {
            Motion::FindForward(c) => Motion::FindBackward(c),
            Motion::FindBackward(c) => Motion::FindForward(c),
            Motion::TillForward(c) => Motion::TillBackward(c),
            Motion::TillBackward(c) => Motion::TillForward(c),
            _ => return None,
        })
    }

    pub fn is_find(self) -> bool {
        matches!(
            self,
            Motion::FindForward(_)
                | Motion::FindBackward(_)
                | Motion::TillForward(_)
                | Motion::TillBackward(_)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertPos {
    Current,
    After,
    LineStart,
    LineEnd,
    LineAbove,
    LineBelow,
}

/// Vim's motion classes. Determines how an operator turns the post-motion
/// cursor into a deletion range:
/// - `Exclusive`: range is `[min, max)` — `dw`, `d0`, `dh`, …
/// - `Inclusive`: range is `[min, max + 1)` — `d$`, `de`, `df<c>`, …
/// - `Linewise`: operates on whole lines — `dj`, `dk`, `dG`, `dgg`, …
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionClass {
    Exclusive,
    Inclusive,
    Linewise,
}

impl Motion {
    pub fn class(self) -> MotionClass {
        match self {
            Motion::Left
            | Motion::Right
            | Motion::LineStart
            | Motion::FirstNonBlank
            | Motion::WordForward
            | Motion::WordBackward => MotionClass::Exclusive,
            Motion::LineEnd
            | Motion::WordEnd
            | Motion::FindForward(_)
            | Motion::FindBackward(_)
            | Motion::TillForward(_)
            | Motion::TillBackward(_) => MotionClass::Inclusive,
            Motion::Up
            | Motion::Down
            | Motion::HalfPageDown
            | Motion::HalfPageUp
            | Motion::DocStart
            | Motion::DocEnd
            | Motion::GotoLine(_) => MotionClass::Linewise,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    Delete,
    Change,
    Yank,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PastePos {
    /// `p` — after cursor (charwise) or below current line (linewise).
    After,
    /// `P` — at cursor (charwise) or above current line (linewise).
    Before,
}

/// Text-object kinds supported in round 3.
///
/// `around == true` matches `a<x>` (includes delimiters / trailing
/// whitespace); `around == false` matches `i<x>` (just the inner text).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextObject {
    /// `iw` / `aw` — run of word-class chars under the cursor.
    Word { around: bool },
    /// `i"` / `a"` / `i'` / `a'` / `` i` `` / `` a` `` — string between
    /// matching delimiters on the same line.
    Quote { delim: char, around: bool },
    /// `i(` / `a(` (also `b`), `i{` / `a{` (also `B`), `i[` / `a[`,
    /// `i<` / `a<` — text between balanced bracket pairs (nested).
    Pair {
        open: char,
        close: char,
        around: bool,
    },
}

/// Map `(around, target_char)` to a [`TextObject`]. The four target
/// chars per pair (open, close, alias) all resolve to the same object,
/// matching vim.
pub fn build_textobject(around: bool, target: char) -> Option<TextObject> {
    Some(match target {
        'w' => TextObject::Word { around },
        '"' | '\'' | '`' => TextObject::Quote {
            delim: target,
            around,
        },
        '(' | ')' | 'b' => TextObject::Pair {
            open: '(',
            close: ')',
            around,
        },
        '{' | '}' | 'B' => TextObject::Pair {
            open: '{',
            close: '}',
            around,
        },
        '[' | ']' => TextObject::Pair {
            open: '[',
            close: ']',
            around,
        },
        '<' | '>' => TextObject::Pair {
            open: '<',
            close: '>',
            around,
        },
        _ => return None,
    })
}

/// Where `zz` / `zt` / `zb` should park the cursor's line within
/// the pane's vertical viewport. Center is half the height; Top
/// hugs the topmost row; Bottom hugs the bottom row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollPos {
    Center,
    Top,
    Bottom,
}

/// Suffix command after the `Ctrl+W` window prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowCmd {
    /// `<C-w>v` — split focused pane vertically (side-by-side).
    SplitVertical,
    /// `<C-w>s` — split focused pane horizontally (top / bottom).
    SplitHorizontal,
    /// `<C-w>h` — focus the pane to the left.
    FocusLeft,
    /// `<C-w>l` — focus the pane to the right.
    FocusRight,
    /// `<C-w>k` — focus the pane above.
    FocusUp,
    /// `<C-w>j` — focus the pane below.
    FocusDown,
    /// `<C-w>w` / `<C-w><C-w>` — cycle focus through leaves.
    Cycle,
    /// `<C-w>c` / `<C-w>q` — close the focused pane (closes the tab when
    /// it was the only pane left).
    Close,
    /// `<C-w>=` — equalize all split ratios in the active tab.
    Equalize,
}

/// Abstract operations every line-edit prompt understands. Mapped to
/// concrete `Action` variants by the per-mode parser callbacks.
pub enum LineEditAction {
    Cancel,
    Execute,
    Char(char),
    Backspace,
    Delete,
    CursorLeft,
    CursorRight,
    CursorHome,
    CursorEnd,
}
