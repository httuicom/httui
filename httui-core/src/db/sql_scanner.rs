//! SQL-aware scanner shared by multi-statement detection, placeholder
//! normalization, and bind count.
//!
//! Extracted from `db::connections`. The scanner tracks
//! whether the cursor is inside a single-
//! quoted string, a `--` line comment, or a `/* ... */` block comment
//! so semicolon-/`?`-aware passes can ignore characters that look
//! like statement boundaries or placeholders but aren't.

/// Cursor state for a streaming SQL scan. Single-quote / line comment
/// / block comment / escape-next are all tracked so the next-char
/// classification is correct without re-parsing.
pub(super) struct SqlScanner {
    in_single_quote: bool,
    in_block_comment: bool,
    in_line_comment: bool,
    escape_next: bool,
}

impl SqlScanner {
    pub(super) fn new() -> Self {
        Self {
            in_single_quote: false,
            in_block_comment: false,
            in_line_comment: false,
            escape_next: false,
        }
    }

    pub(super) fn is_code(&self) -> bool {
        !self.in_single_quote && !self.in_block_comment && !self.in_line_comment
    }

    /// Advance state by one character. Returns how many chars were consumed (1 or 2).
    pub(super) fn advance(&mut self, ch: char, next: Option<char>) -> usize {
        if self.escape_next {
            self.escape_next = false;
            return 1;
        }

        if self.in_line_comment {
            if ch == '\n' {
                self.in_line_comment = false;
            }
            return 1;
        }

        if self.in_block_comment {
            if ch == '*' && next == Some('/') {
                self.in_block_comment = false;
                return 2;
            }
            return 1;
        }

        if ch == '\\' && self.in_single_quote {
            self.escape_next = true;
            return 1;
        }

        if ch == '-' && next == Some('-') && !self.in_single_quote {
            self.in_line_comment = true;
            return 1;
        }

        if ch == '/' && next == Some('*') && !self.in_single_quote {
            self.in_block_comment = true;
            return 2;
        }

        if ch == '\'' {
            self.in_single_quote = !self.in_single_quote;
        }

        1
    }
}

/// Multi-statement detection — returns `true` when the SQL contains
/// a `;` boundary in code (not in strings or comments) followed by
/// non-empty content. A trailing semicolon with only whitespace after
/// returns `false` (legal single-statement input).
pub(crate) fn contains_multiple_statements(sql: &str) -> bool {
    let chars: Vec<char> = sql.chars().collect();
    let len = chars.len();
    let mut scanner = SqlScanner::new();
    let mut i = 0;

    while i < len {
        let ch = chars[i];
        let next = chars.get(i + 1).copied();

        let was_code = scanner.is_code();
        let consumed = scanner.advance(ch, next);

        if ch == ';' && was_code && scanner.is_code() {
            // Trailing semicolon with only whitespace after is OK
            let rest: String = chars[i + 1..].iter().collect();
            if !rest.trim().is_empty() {
                return true;
            }
            return false;
        }

        i += consumed;
    }
    false
}

/// Split a SQL string on `;` boundaries that appear in code (not in strings,
/// line comments, or block comments). Drops statements that are empty after
/// trimming — callers get back exactly the statements they need to execute,
/// in source order. A single-statement input returns a single-element Vec.
pub fn split_statements(sql: &str) -> Vec<String> {
    let chars: Vec<char> = sql.chars().collect();
    let len = chars.len();
    let mut scanner = SqlScanner::new();
    let mut i = 0;
    let mut current = String::new();
    let mut statements: Vec<String> = Vec::new();

    while i < len {
        let ch = chars[i];
        let next = chars.get(i + 1).copied();

        let was_code = scanner.is_code();
        let consumed = scanner.advance(ch, next);

        if ch == ';' && was_code && scanner.is_code() {
            // Boundary — emit current statement (without the `;`).
            let trimmed = current.trim();
            if !trimmed.is_empty() {
                statements.push(trimmed.to_string());
            }
            current.clear();
            i += consumed;
            continue;
        }

        // Copy the consumed chars into the current statement buffer.
        for j in 0..consumed {
            if i + j < len {
                current.push(chars[i + j]);
            }
        }
        i += consumed;
    }

    let trailing = current.trim();
    if !trailing.is_empty() {
        statements.push(trailing.to_string());
    }

    statements
}

/// Convert `?` placeholders to `$N` for Postgres.
/// Skips `?` inside string literals, block comments, and line comments.
pub fn normalize_placeholders_to_pg(sql: &str) -> String {
    let chars: Vec<char> = sql.chars().collect();
    let len = chars.len();
    let mut result = String::with_capacity(sql.len());
    let mut counter = 0u32;
    let mut scanner = SqlScanner::new();
    let mut i = 0;

    while i < len {
        let ch = chars[i];
        let next = chars.get(i + 1).copied();

        let is_code = scanner.is_code();
        let consumed = scanner.advance(ch, next);

        if ch == '?' && is_code {
            counter += 1;
            result.push('$');
            result.push_str(&counter.to_string());
            i += consumed;
            continue;
        }

        // Push all consumed chars
        for j in 0..consumed {
            if i + j < len {
                result.push(chars[i + j]);
            }
        }
        i += consumed;
    }

    result
}

/// Count `?` placeholders in code (skipping strings/comments). Used to
/// validate that `bind_values.len() == count_placeholders(sql)`.
pub fn count_placeholders(sql: &str) -> usize {
    let chars: Vec<char> = sql.chars().collect();
    let len = chars.len();
    let mut count = 0;
    let mut scanner = SqlScanner::new();
    let mut i = 0;

    while i < len {
        let ch = chars[i];
        let next = chars.get(i + 1).copied();

        let is_code = scanner.is_code();
        let consumed = scanner.advance(ch, next);

        if ch == '?' && is_code {
            count += 1;
        }

        i += consumed;
    }

    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_statements_single() {
        assert_eq!(split_statements("SELECT 1"), vec!["SELECT 1"]);
    }

    #[test]
    fn split_statements_multi_with_trailing_semicolon() {
        let s = split_statements("SELECT 1; SELECT 2;");
        assert_eq!(s, vec!["SELECT 1", "SELECT 2"]);
    }

    #[test]
    fn split_statements_ignores_semicolon_in_string() {
        let s = split_statements("SELECT 'a;b'; SELECT 2");
        assert_eq!(s, vec!["SELECT 'a;b'", "SELECT 2"]);
    }

    #[test]
    fn split_statements_ignores_semicolon_in_line_comment() {
        let s = split_statements("SELECT 1 -- ;\n; SELECT 2");
        assert_eq!(s.len(), 2);
        assert!(s[0].starts_with("SELECT 1"));
    }

    #[test]
    fn contains_multiple_statements_detects_real_boundaries() {
        assert!(contains_multiple_statements("SELECT 1; SELECT 2"));
        assert!(!contains_multiple_statements("SELECT 1"));
        assert!(!contains_multiple_statements("SELECT 1;"));
        assert!(!contains_multiple_statements("SELECT 'a;b'"));
    }

    #[test]
    fn normalize_placeholders_to_pg_basic() {
        assert_eq!(
            normalize_placeholders_to_pg("SELECT ?, ?, ?"),
            "SELECT $1, $2, $3"
        );
    }

    #[test]
    fn normalize_placeholders_to_pg_skips_strings_and_comments() {
        assert_eq!(
            normalize_placeholders_to_pg("SELECT '?' /* ? */ -- ?\nFROM t WHERE a = ?"),
            "SELECT '?' /* ? */ -- ?\nFROM t WHERE a = $1"
        );
    }

    #[test]
    fn count_placeholders_skips_strings() {
        assert_eq!(count_placeholders("SELECT ?, ?, ?"), 3);
        assert_eq!(count_placeholders("SELECT '?', ?"), 1);
        assert_eq!(count_placeholders("/* ? */ SELECT 1"), 0);
    }
}
