//! IDE-style bracket/quote pairing decisions. Pure char logic — the
//! applier reads the chars around the cursor, asks this module what
//! the keystroke means, and performs the edits. Only applied in code
//! contexts (block bodies, BLOCKS-view edit fields); prose never
//! pairs, so apostrophes in text stay single keystrokes.

/// What a typed character should do under auto-pairing.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PairOutcome {
    /// Insert the typed char AND this closer, caret between them.
    Pair(char),
    /// The next char already is the typed closer — step over it
    /// instead of inserting a duplicate.
    Skip,
    /// Plain insert, no pairing.
    Plain,
}

fn closer_for(opener: char) -> Option<char> {
    match opener {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        '"' => Some('"'),
        '\'' => Some('\''),
        '`' => Some('`'),
        _ => None,
    }
}

fn is_closer(c: char) -> bool {
    matches!(c, ')' | ']' | '}')
}

fn is_quote(c: char) -> bool {
    matches!(c, '"' | '\'' | '`')
}

fn is_wordish(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || c == '_')
}

/// Decision for typing `c` with `prev`/`next` being the chars around
/// the caret.
pub(crate) fn on_insert(c: char, prev: Option<char>, next: Option<char>) -> PairOutcome {
    // Stepping over the closer one just typed past — covers both
    // bracket closers and the closing half of a quote pair.
    if (is_closer(c) || is_quote(c)) && next == Some(c) {
        return PairOutcome::Skip;
    }
    if is_quote(c) {
        // A quote right after a word char is an apostrophe / string
        // continuation (`don't`, `it's`), and pairing before a word
        // would swallow it (`"foo` → `"f"oo`). Both stay plain.
        if is_wordish(prev) || is_wordish(next) {
            return PairOutcome::Plain;
        }
        return PairOutcome::Pair(c);
    }
    if let Some(closer) = closer_for(c) {
        // Opening bracket directly before a word would wrap nothing
        // useful and surprises mid-edit typing (`{`foo → `{}foo`).
        if is_wordish(next) {
            return PairOutcome::Plain;
        }
        return PairOutcome::Pair(closer);
    }
    PairOutcome::Plain
}

/// Backspace between an empty pair removes both halves.
pub(crate) fn deletes_pair(prev: Option<char>, next: Option<char>) -> bool {
    match (prev, next) {
        (Some(p), Some(n)) => closer_for(p) == Some(n),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openers_pair_with_their_closers() {
        assert_eq!(on_insert('{', None, None), PairOutcome::Pair('}'));
        assert_eq!(on_insert('(', Some(' '), None), PairOutcome::Pair(')'));
        assert_eq!(on_insert('[', Some('='), Some(' ')), PairOutcome::Pair(']'));
    }

    #[test]
    fn opener_before_word_stays_plain() {
        assert_eq!(on_insert('{', None, Some('f')), PairOutcome::Plain);
        assert_eq!(on_insert('(', Some(' '), Some('x')), PairOutcome::Plain);
    }

    #[test]
    fn closer_over_matching_next_skips() {
        assert_eq!(on_insert('}', Some('{'), Some('}')), PairOutcome::Skip);
        assert_eq!(on_insert(')', Some('a'), Some(')')), PairOutcome::Skip);
        assert_eq!(on_insert(']', None, Some(']')), PairOutcome::Skip);
    }

    #[test]
    fn closer_without_matching_next_is_plain() {
        assert_eq!(on_insert('}', Some('a'), Some(')')), PairOutcome::Plain);
        assert_eq!(on_insert(')', None, None), PairOutcome::Plain);
    }

    #[test]
    fn quotes_pair_in_neutral_context() {
        assert_eq!(on_insert('"', None, None), PairOutcome::Pair('"'));
        assert_eq!(on_insert('\'', Some(' '), None), PairOutcome::Pair('\''));
        assert_eq!(on_insert('`', Some('('), Some(')')), PairOutcome::Pair('`'));
    }

    #[test]
    fn quote_after_word_char_is_an_apostrophe() {
        assert_eq!(on_insert('\'', Some('n'), Some('t')), PairOutcome::Plain);
        assert_eq!(on_insert('"', Some('x'), None), PairOutcome::Plain);
    }

    #[test]
    fn quote_before_word_char_stays_plain() {
        assert_eq!(on_insert('"', Some(' '), Some('f')), PairOutcome::Plain);
    }

    #[test]
    fn quote_over_its_own_closer_skips() {
        assert_eq!(on_insert('"', Some('a'), Some('"')), PairOutcome::Skip);
        assert_eq!(on_insert('\'', None, Some('\'')), PairOutcome::Skip);
    }

    #[test]
    fn backspace_inside_empty_pair_deletes_both() {
        assert!(deletes_pair(Some('{'), Some('}')));
        assert!(deletes_pair(Some('('), Some(')')));
        assert!(deletes_pair(Some('"'), Some('"')));
        assert!(deletes_pair(Some('\''), Some('\'')));
    }

    #[test]
    fn backspace_elsewhere_deletes_one() {
        assert!(!deletes_pair(Some('{'), Some(')')));
        assert!(!deletes_pair(Some('a'), Some('}')));
        assert!(!deletes_pair(None, Some('}')));
        assert!(!deletes_pair(Some('{'), None));
    }
}
