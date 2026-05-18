//! SQL completion engine for the TUI.
//!
//! a — the popup infra + the keyword/builtin sources that work
//! without any schema knowledge. b adds the schema
//! source (tables/columns) on top, and 04.7 adds the `{{refs}}`
//! source. All three plug into the same `CompletionItem` shape and
//! popup widget.
//!
//! Why do we hand-roll keyword lists when desktop reuses
//! `@codemirror/lang-sql`? The TUI doesn't have a SQL grammar with a
//! token table we can just expose. tree-sitter's SQL grammar exists
//! in the project (cached in `ui::sql_highlight`) but its node-kinds
//! don't match a stable keyword set. Hard-coding ~80 keywords plus
//! per-dialect builtins is small, fast, and easy to extend.
//!
//! Filter is case-insensitive prefix match — the popup re-runs on
//! every keystroke so we don't need fuzzy here. Items sort
//! alphabetically by label, with category as the tie-breaker.

use crate::buffer::block::BlockNode;
use crate::schema::SchemaTable;

/// One row in the completion popup. The same shape is returned by
/// every source (keywords, schema, refs) so the popup widget renders
/// uniformly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    /// What gets inserted when the user accepts.
    pub label: String,
    /// Category — drives the dim suffix in the popup.
    pub kind: CompletionKind,
    /// Optional secondary string. b sets this to the column
    /// type (e.g. `text`, `int4`); 04.7 to `cached`/`no-result`.
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Keyword,
    Function,
    Table,
    Column,
    /// Reserved for `{{ref}}` autocomplete.
    #[allow(dead_code)]
    Reference,
}

impl CompletionKind {
    pub fn label(self) -> &'static str {
        match self {
            CompletionKind::Keyword => "keyword",
            CompletionKind::Function => "function",
            CompletionKind::Table => "table",
            CompletionKind::Column => "column",
            CompletionKind::Reference => "ref",
        }
    }
}

/// SQL dialect — picked from `block.block_type` (`db-postgres`,
/// `db-mysql`, `db-sqlite`). Drives which builtin list is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    Postgres,
    MySql,
    Sqlite,
    /// Generic fallback — ANSI keywords only, no dialect builtins.
    Generic,
}

impl Dialect {
    pub fn from_block(block: &BlockNode) -> Self {
        match block.block_type.as_str() {
            "db-postgres" => Dialect::Postgres,
            "db-mysql" => Dialect::MySql,
            "db-sqlite" => Dialect::Sqlite,
            _ => Dialect::Generic,
        }
    }
}

/// Wrap a query in the dialect's EXPLAIN keyword so the result
/// contains the planner's execution plan instead of (or alongside)
/// the actual rows. Postgres / MySQL use bare `EXPLAIN <query>`;
/// SQLite uses `EXPLAIN QUERY PLAN <query>` (its plain `EXPLAIN`
/// dumps VDBE bytecode which isn't useful to most users).
///
/// Trailing semicolon is stripped so `EXPLAIN SELECT 1;` reads as
/// `EXPLAIN SELECT 1` — the executor's multi-statement splitter
/// would otherwise see two statements (the EXPLAIN and an empty one)
/// and emit a confusing extra result.
///
/// V1 only wraps the *first* statement when a multi-statement query
/// is passed — explaining each individually is V2.
pub fn explain_wrap(query: &str, dialect: Dialect) -> String {
    // Find the first `;` outside of string / comment context. V1
    // is naive and just splits on the first literal `;` — same
    // approximation `is_unscoped_destructive` uses. Good enough
    // for the common case (single SELECT / UPDATE).
    let trimmed = query.trim();
    let first_stmt = match trimmed.find(';') {
        Some(idx) => trimmed[..idx].trim(),
        None => trimmed,
    };
    let prefix = match dialect {
        Dialect::Sqlite => "EXPLAIN QUERY PLAN ",
        Dialect::Postgres | Dialect::MySql | Dialect::Generic => "EXPLAIN ",
    };
    format!("{prefix}{first_stmt}")
}

/// ANSI-ish keyword set shared across dialects. Covers the 80% the
/// average note will type. Sorted, deduped, all-uppercase canonical
/// form — the popup match is case-insensitive so `select` still
/// matches `SELECT`.
const ANSI_KEYWORDS: &[&str] = &[
    "ADD",
    "ALL",
    "ALTER",
    "AND",
    "ANALYZE",
    "AS",
    "ASC",
    "BEGIN",
    "BETWEEN",
    "BY",
    "CASCADE",
    "CASE",
    "CAST",
    "CHECK",
    "COLUMN",
    "COMMIT",
    "CONSTRAINT",
    "CREATE",
    "CROSS",
    "DEFAULT",
    "DELETE",
    "DESC",
    "DISTINCT",
    "DROP",
    "ELSE",
    "END",
    "EXCEPT",
    "EXISTS",
    "EXPLAIN",
    "FALSE",
    "FOREIGN",
    "FROM",
    "FULL",
    "GROUP",
    "HAVING",
    "IF",
    "IN",
    "INDEX",
    "INNER",
    "INSERT",
    "INTERSECT",
    "INTO",
    "IS",
    "JOIN",
    "KEY",
    "LEFT",
    "LIKE",
    "LIMIT",
    "NOT",
    "NULL",
    "OFFSET",
    "ON",
    "OR",
    "ORDER",
    "OUTER",
    "PRIMARY",
    "REFERENCES",
    "RIGHT",
    "ROLLBACK",
    "SELECT",
    "SET",
    "TABLE",
    "THEN",
    "TRUE",
    "UNION",
    "UNIQUE",
    "UPDATE",
    "USING",
    "VALUES",
    "VIEW",
    "WHEN",
    "WHERE",
    "WITH",
];

/// Postgres-flavored extras — keywords + dialect-specific syntax
/// like `RETURNING` and `ILIKE` that aren't in pure ANSI but show up
/// constantly in real notes.
const POSTGRES_KEYWORDS: &[&str] = &["ILIKE", "MATERIALIZED", "RECURSIVE", "RETURNING"];

/// MySQL extras — `IGNORE`, `REPLACE`, etc. Conservative list; the
/// engine accepts anything but we keep the popup focused on what
/// users actually type.
const MYSQL_KEYWORDS: &[&str] = &["IGNORE", "REPLACE", "STRAIGHT_JOIN"];

/// SQLite extras — `PRAGMA` is the big one; the rest mirror common
/// dialect-specific syntax.
const SQLITE_KEYWORDS: &[&str] = &["AUTOINCREMENT", "GLOB", "PRAGMA", "VACUUM"];

/// Postgres function builtins. Curated, not exhaustive — covers
/// aggregates, string manipulation, JSON, and the date/time helpers
/// users reach for daily.
const POSTGRES_FUNCTIONS: &[&str] = &[
    "ABS",
    "AVG",
    "CASE",
    "COALESCE",
    "COUNT",
    "CURRENT_DATE",
    "CURRENT_TIMESTAMP",
    "DATE_PART",
    "DATE_TRUNC",
    "EXTRACT",
    "GENERATE_SERIES",
    "GREATEST",
    "INITCAP",
    "JSONB_BUILD_OBJECT",
    "JSONB_EACH",
    "JSONB_EXTRACT_PATH",
    "LEAST",
    "LENGTH",
    "LOWER",
    "MAX",
    "MIN",
    "NOW",
    "NULLIF",
    "POSITION",
    "REGEXP_REPLACE",
    "REPLACE",
    "ROUND",
    "ROW_NUMBER",
    "STRING_AGG",
    "SUBSTRING",
    "SUM",
    "TO_CHAR",
    "TO_DATE",
    "TO_TIMESTAMP",
    "TRIM",
    "UPPER",
];

/// MySQL function builtins.
const MYSQL_FUNCTIONS: &[&str] = &[
    "ABS",
    "AVG",
    "CONCAT",
    "CONCAT_WS",
    "COUNT",
    "CURDATE",
    "CURRENT_DATE",
    "CURRENT_TIMESTAMP",
    "DATE_ADD",
    "DATE_FORMAT",
    "DATE_SUB",
    "DAY",
    "EXTRACT",
    "GREATEST",
    "GROUP_CONCAT",
    "HOUR",
    "IF",
    "IFNULL",
    "JSON_EXTRACT",
    "JSON_OBJECT",
    "LEAST",
    "LENGTH",
    "LOWER",
    "MAX",
    "MIN",
    "MONTH",
    "NOW",
    "NULLIF",
    "REPLACE",
    "ROUND",
    "ROW_NUMBER",
    "SUBSTRING",
    "SUM",
    "TIMESTAMP",
    "TRIM",
    "UPPER",
    "YEAR",
];

/// SQLite function builtins. Smaller list — SQLite has fewer
/// builtins. Includes `JSON_EXTRACT` for the JSON1 extension which
/// is commonly enabled.
const SQLITE_FUNCTIONS: &[&str] = &[
    "ABS",
    "AVG",
    "CASE",
    "COALESCE",
    "COUNT",
    "DATE",
    "DATETIME",
    "GROUP_CONCAT",
    "IFNULL",
    "JSON_EXTRACT",
    "JULIANDAY",
    "LENGTH",
    "LIKELY",
    "LOWER",
    "MAX",
    "MIN",
    "NULLIF",
    "PRINTF",
    "RANDOM",
    "REPLACE",
    "ROUND",
    "STRFTIME",
    "SUBSTR",
    "SUM",
    "TIME",
    "TRIM",
    "TYPEOF",
    "UNLIKELY",
    "UPPER",
];

/// What the cursor's surrounding SQL is asking for. The dispatcher
/// computes this from the body left of the cursor; the engine uses
/// it to decide whether to surface schema items, and which kind.
///
/// The detector handles three explicit cases plus a "general" one:
/// `FROM`/`JOIN`/`INTO`/`UPDATE` → table; `<word>.` → columns of
/// that word; anything else → `Open` carrying the *tables already in
/// scope* (extracted from `FROM`/`JOIN` clauses elsewhere in the SQL)
/// so bare column names also surface mid-`SELECT`/`WHERE`/`ON`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqlContext {
    /// Mid-statement — keywords/builtins plus, when `in_scope` is
    /// non-empty, columns from the named tables. Lets users type
    /// `WHERE i⌷` and get `id` (without spelling out `users.id`).
    /// `in_scope` may include tables not present in the schema cache;
    /// the engine just skips those.
    Open { in_scope: Vec<String> },
    /// User is naming a table next: `FROM ⌷`, `JOIN ⌷`, `INTO ⌷`,
    /// `UPDATE ⌷`. Schema source contributes table names; keywords
    /// and builtins still appear (a subquery start with `SELECT` is
    /// legal here too).
    Table,
    /// User is naming a column on a known table: `users.⌷`. Schema
    /// source contributes that table's columns; keywords/builtins
    /// don't make sense after `<table>.` so they're suppressed.
    ColumnOf(String),
}

impl SqlContext {
    /// Convenience for tests / call sites that don't care about
    /// scope. Returns `Open { in_scope: vec![] }`.
    pub fn open_no_scope() -> Self {
        SqlContext::Open {
            in_scope: Vec::new(),
        }
    }
}

/// Walk the SQL left of the cursor and decide what category of
/// completion to surface. `anchor_offset` is where the *prefix*
/// word starts (same as `prefix_at_cursor`'s first return); we look
/// at what comes *before* that. Multi-line walk only on the current
/// line for V1 — `FROM` on a previous line still works most of the
/// time because users tend to put `FROM` and the table name on the
/// same line.
pub fn detect_context(body: &str, line: usize, anchor_offset: usize) -> SqlContext {
    let line_text = match body.lines().nth(line) {
        Some(s) => s,
        None => return SqlContext::open_no_scope(),
    };
    let chars: Vec<char> = line_text.chars().collect();
    let take = anchor_offset.min(chars.len());
    let head: String = chars[..take].iter().collect();

    // Trailing dot? `<word>.` → ColumnOf(<word>).
    if head.ends_with('.') {
        let body_no_dot = &head[..head.len() - 1];
        // Walk back through `[A-Za-z0-9_]+` to extract the word.
        let table_start = body_no_dot
            .rfind(|c: char| !is_word_char(c))
            .map(|i| i + 1)
            .unwrap_or(0);
        let table = &body_no_dot[table_start..];
        if !table.is_empty() {
            return SqlContext::ColumnOf(table.to_string());
        }
        return SqlContext::open_no_scope();
    }

    // Trailing whitespace before the prefix → look at the last word
    // before the gap. `FROM` / `JOIN` / `INTO` (after INSERT) /
    // `UPDATE` open a table-naming spot.
    let trimmed = head.trim_end_matches(|c: char| c.is_whitespace());
    if trimmed.len() != head.len() {
        let last_word_start = trimmed
            .rfind(|c: char| !is_word_char(c))
            .map(|i| i + 1)
            .unwrap_or(0);
        let last_word = &trimmed[last_word_start..];
        let upper = last_word.to_ascii_uppercase();
        if matches!(upper.as_str(), "FROM" | "JOIN" | "UPDATE" | "INTO") {
            return SqlContext::Table;
        }
    }

    // Mid-statement — extract tables from `FROM`/`JOIN` clauses
    // anywhere in the SQL so bare column names also surface in
    // `WHERE`/`ON`/`SELECT` positions. The detector looks at the
    // whole body (not just the current line) because users typically
    // put `FROM` on its own line above the `WHERE`.
    SqlContext::Open {
        in_scope: extract_tables_in_scope(body),
    }
}

/// Pull table names out of `FROM ⌷` and `JOIN ⌷` positions in the
/// SQL. Used by `detect_context` to populate the scope of `Open`
/// contexts so columns from those tables surface as bare names.
///
/// V1 limits: ignores quoted identifiers, comments, and string
/// literals (could mistake `'FROM users'` inside a quoted string for
/// a real `FROM`). Acceptable trade-off for a popup heuristic — the
/// worst case is a spurious column suggestion, never a wrong query.
pub fn extract_tables_in_scope(body: &str) -> Vec<String> {
    let chars: Vec<char> = body.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        // Look for an alphabetic word starting here.
        if !is_word_char(chars[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && is_word_char(chars[i]) {
            i += 1;
        }
        let word: String = chars[start..i].iter().collect();
        // Word boundary check on the left — `start == 0` or the
        // previous char is non-word.
        let left_ok = start == 0 || !is_word_char(chars[start - 1]);
        if !left_ok {
            continue;
        }
        let upper = word.to_ascii_uppercase();
        if upper != "FROM" && upper != "JOIN" {
            continue;
        }
        // Skip whitespace, then take the table name (next word).
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        let tbl_start = i;
        while i < chars.len() && is_word_char(chars[i]) {
            i += 1;
        }
        if i > tbl_start {
            let table: String = chars[tbl_start..i].iter().collect();
            // Skip pseudo-keywords that can follow FROM/JOIN in some
            // dialects (`SELECT` in subqueries, `LATERAL` modifier).
            let table_upper = table.to_ascii_uppercase();
            if !matches!(table_upper.as_str(), "SELECT" | "LATERAL")
                && !out.iter().any(|t| t.eq_ignore_ascii_case(&table))
            {
                out.push(table);
            }
        }
    }
    out
}

/// Build the candidate list for the popup. `prefix` is the partial
/// word the user has typed; `context` is what the cursor's
/// surroundings hint at; `schema` is the in-memory schema cache for
/// the active connection (or `None` when not yet loaded). Schema
/// items lead, then keywords/builtins — except in `ColumnOf`, which
/// suppresses keywords entirely (a column name slot can't take a
/// keyword anyway).
///
/// Sorted alphabetically by label so the same prefix always produces
/// the same popup ordering — UX wins from determinism here.
pub fn complete(
    dialect: Dialect,
    prefix: &str,
    context: SqlContext,
    schema: Option<&[SchemaTable]>,
) -> Vec<CompletionItem> {
    let prefix_upper = prefix.to_ascii_uppercase();
    let mut out: Vec<CompletionItem> = Vec::new();

    // Schema source — only when we have a cache for this connection
    // and the context tells us what to surface.
    if let Some(tables) = schema {
        match &context {
            SqlContext::Table => {
                for t in tables {
                    if t.name.to_ascii_uppercase().starts_with(&prefix_upper) {
                        out.push(CompletionItem {
                            label: t.name.clone(),
                            kind: CompletionKind::Table,
                            detail: t.schema.clone(),
                        });
                    }
                }
            }
            SqlContext::ColumnOf(table_name) => {
                // Match the table name case-insensitively — users
                // often type `users.id` even if the schema name is
                // `Users` or quoted differently. V1 ignores aliases
                // (no scope analysis); a future story will track
                // `FROM users u` → alias `u` resolves to `users`.
                if let Some(table) = tables
                    .iter()
                    .find(|t| t.name.eq_ignore_ascii_case(table_name))
                {
                    for col in &table.columns {
                        if col.name.to_ascii_uppercase().starts_with(&prefix_upper) {
                            out.push(CompletionItem {
                                label: col.name.clone(),
                                kind: CompletionKind::Column,
                                detail: col.data_type.clone(),
                            });
                        }
                    }
                }
                // ColumnOf suppresses keywords/builtins — return now
                // so the sort below sees only column items.
                out.sort_by(|a, b| a.label.cmp(&b.label));
                out.dedup_by(|a, b| a.label == b.label);
                return out;
            }
            SqlContext::Open { in_scope } => {
                // Bare column completion — columns of every table in
                // scope (extracted from FROM/JOIN clauses) get added
                // alongside keywords/builtins. Detail line shows
                // `from <table>` so two tables sharing a column name
                // (`id` in both `users` and `orders`) stay
                // disambiguated in the popup.
                for table_name in in_scope {
                    if let Some(table) = tables
                        .iter()
                        .find(|t| t.name.eq_ignore_ascii_case(table_name))
                    {
                        for col in &table.columns {
                            if col.name.to_ascii_uppercase().starts_with(&prefix_upper) {
                                out.push(CompletionItem {
                                    label: col.name.clone(),
                                    kind: CompletionKind::Column,
                                    detail: Some(format!("from {}", table.name)),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    let keyword_lists: &[&[&str]] = match dialect {
        Dialect::Postgres => &[ANSI_KEYWORDS, POSTGRES_KEYWORDS],
        Dialect::MySql => &[ANSI_KEYWORDS, MYSQL_KEYWORDS],
        Dialect::Sqlite => &[ANSI_KEYWORDS, SQLITE_KEYWORDS],
        Dialect::Generic => &[ANSI_KEYWORDS],
    };
    for list in keyword_lists {
        for kw in *list {
            if kw.starts_with(&prefix_upper) {
                out.push(CompletionItem {
                    label: (*kw).to_string(),
                    kind: CompletionKind::Keyword,
                    detail: None,
                });
            }
        }
    }

    let fn_list: &[&str] = match dialect {
        Dialect::Postgres => POSTGRES_FUNCTIONS,
        Dialect::MySql => MYSQL_FUNCTIONS,
        Dialect::Sqlite => SQLITE_FUNCTIONS,
        Dialect::Generic => &[],
    };
    for fname in fn_list {
        if fname.starts_with(&prefix_upper) {
            out.push(CompletionItem {
                label: (*fname).to_string(),
                kind: CompletionKind::Function,
                detail: None,
            });
        }
    }

    // Stable sort: alphabetical by label, with kind as a deterministic
    // tie-breaker. The `Ord` impl below sets `Keyword < Function`, so
    // a token that exists in both lists (`CASE`, `COUNT`) keeps the
    // keyword variant when we dedup below.
    out.sort_by(|a, b| a.label.cmp(&b.label).then_with(|| a.kind.cmp(&b.kind)));
    // Dedup by label only — popup never shows the same word twice.
    // The tie-break in the sort above means we keep the keyword
    // variant when both kinds match, which is the more useful
    // categorization for the user.
    out.dedup_by(|a, b| a.label == b.label);
    out
}

// `Ord` for `CompletionKind` so the dedup tie-breaker is well-defined.
// Order is informational only (not user-facing).
impl PartialOrd for CompletionKind {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CompletionKind {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}

/// What the detector found inside an open `{{...}}` ref. Returned
/// by `detect_ref_context` when the cursor sits between an opener
/// and the matching `}}`. The completion engine uses it to switch
/// off the SQL path entirely (refs and SQL keywords don't mix).
///
/// Splitting on `.` matters because the engine surfaces *different*
/// items per segment:
/// - segment 1 (`{{|}}` or `{{q1|}}`) → alias names + env vars
/// - segment 2+ (`{{q1.|}}` or `{{q1.id|}}`) → keys of that
///   block's cached result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefDetect {
    /// Where the *prefix* word starts in the body (column on the
    /// current line). Used by the popup's accept handler to know
    /// how many characters to backspace before splicing the chosen
    /// label.
    pub anchor_offset: usize,
    /// What the user has typed since the last `{{` or `.` — the
    /// item the popup will filter on. Empty when the cursor sits
    /// right after `{{` or `.`.
    pub prefix: String,
    /// Path segments before the current one. `None` for the first
    /// segment after `{{`; `Some("q1")` for `{{q1.|}}`;
    /// `Some("q1.response")` for `{{q1.response.|}}`.
    pub path: Option<String>,
}

/// Detect whether the cursor sits inside an open `{{...}}` ref. V1
/// only walks the current line — refs spanning multiple lines are
/// rare in practice and would complicate detection a lot. Returns
/// `None` when there's no open `{{` to the left, or when there's
/// already a `}}` between the opener and the cursor (the ref is
/// closed; we're back in plain SQL).
pub fn detect_ref_context(body: &str, line: usize, cursor_offset: usize) -> Option<RefDetect> {
    let line_text = body.lines().nth(line)?;
    let chars: Vec<char> = line_text.chars().collect();
    let take = cursor_offset.min(chars.len());
    let head: String = chars[..take].iter().collect();

    // Find the *last* `{{` to the left, then make sure no `}}`
    // appears between it and the cursor. If a `}}` is present, the
    // ref is closed and we're back in plain SQL.
    let last_open = head.rfind("{{")?;
    let after_open = &head[last_open + 2..];
    if after_open.contains("}}") {
        return None;
    }

    // The current segment is everything since the last `.` (or the
    // whole `after_open` when there's no dot yet). The path is
    // everything before that dot.
    let (path, prefix) = match after_open.rfind('.') {
        Some(dot_idx) => (
            Some(after_open[..dot_idx].to_string()),
            after_open[dot_idx + 1..].to_string(),
        ),
        None => (None, after_open.to_string()),
    };

    let anchor_offset = cursor_offset.saturating_sub(prefix.chars().count());
    Some(RefDetect {
        anchor_offset,
        prefix,
        path,
    })
}

/// Build the candidate list for the ref popup. When `detect.path`
/// is `None`, surfaces aliases of blocks above `current_segment`
/// plus env vars from the active environment.
///
/// When `detect.path` is set, walks a synthetic
/// `{response: <cached>, status: "..."}` envelope — same shape the
/// desktop builds in `references.ts:140-143` — and emits the keys
/// of whatever value the path lands on. Pure JSON walk: arrays
/// contribute their numeric indices, objects their keys, primitives
/// nothing (popup closes). The legacy `{{alias.col}}` first-row
/// shim is *not* surfaced here — it's a runtime resolver shim, not
/// an autocomplete suggestion.
pub fn complete_refs(
    detect: &RefDetect,
    segments: &[crate::buffer::Segment],
    current_segment: usize,
    env_vars: &std::collections::HashMap<String, String>,
) -> Vec<CompletionItem> {
    let prefix_lower = detect.prefix.to_ascii_lowercase();
    let mut out: Vec<CompletionItem> = Vec::new();

    let Some(path) = detect.path.as_deref() else {
        // Top-level: aliases first (most-typed), env vars after.
        for seg in segments.iter().take(current_segment) {
            if let crate::buffer::Segment::Block(b) = seg {
                if let Some(alias) = b.alias.as_deref() {
                    if alias.to_ascii_lowercase().starts_with(&prefix_lower) {
                        let cached = if b.cached_result.is_some() {
                            "cached"
                        } else {
                            "no result"
                        };
                        out.push(CompletionItem {
                            label: alias.to_string(),
                            kind: CompletionKind::Reference,
                            detail: Some(format!("{} · {cached}", b.block_type)),
                        });
                    }
                }
            }
        }
        for key in env_vars.keys() {
            if key.to_ascii_lowercase().starts_with(&prefix_lower) {
                out.push(CompletionItem {
                    label: key.clone(),
                    kind: CompletionKind::Reference,
                    detail: Some("env".into()),
                });
            }
        }
        out.sort_by(|a, b| a.label.cmp(&b.label));
        out.dedup_by(|a, b| a.label == b.label);
        return out;
    };

    // Path is set — first segment is the alias.
    let path_segs: Vec<&str> = path.split('.').collect();
    let alias = match path_segs.first() {
        Some(h) => *h,
        None => return out,
    };
    let block = segments
        .iter()
        .take(current_segment)
        .filter_map(|s| match s {
            crate::buffer::Segment::Block(b) => Some(b),
            _ => None,
        })
        .find(|b| b.alias.as_deref() == Some(alias));
    let Some(block) = block else { return out };
    let Some(cached) = block.cached_result.as_ref() else {
        return out;
    };

    // Synthesize the navigation envelope — matches desktop's
    // `references.ts:140-143`. The autocomplete walks *this* shape,
    // not `cached_result` directly, so `{{alias.|}}` shows
    // `response` + `status` (the envelope's keys), not the keys of
    // the underlying response.
    let status_str = match &block.state {
        crate::buffer::block::ExecutionState::Success
        | crate::buffer::block::ExecutionState::Cached => "success",
        crate::buffer::block::ExecutionState::Error(_) => "error",
        crate::buffer::block::ExecutionState::Running => "running",
        crate::buffer::block::ExecutionState::Idle => "idle",
    };
    let synthetic_root = serde_json::json!({
        "response": cached,
        "status": status_str,
    });

    // Walk every path segment after the alias against the synthetic
    // root. Arrays support both string-key (`.results`) and numeric
    // index (`.0`) navigation.
    let mut cursor: &serde_json::Value = &synthetic_root;
    for seg in &path_segs[1..] {
        let next = cursor.get(seg).or_else(|| {
            seg.parse::<usize>()
                .ok()
                .and_then(|i| cursor.as_array().and_then(|a| a.get(i)))
        });
        match next {
            Some(v) => cursor = v,
            None => return out,
        }
    }

    // Emit the children of `cursor` based on its shape. Detail
    // mirrors desktop's hint format (`Array(N)`, `{N keys}`,
    // `"string"`, `42`, etc.) so the popup feels familiar.
    if let Some(obj) = cursor.as_object() {
        for (key, val) in obj {
            if key.to_ascii_lowercase().starts_with(&prefix_lower) {
                out.push(CompletionItem {
                    label: key.clone(),
                    kind: CompletionKind::Reference,
                    detail: Some(value_hint(val)),
                });
            }
        }
    } else if let Some(arr) = cursor.as_array() {
        for (i, val) in arr.iter().enumerate() {
            let label = i.to_string();
            if label.starts_with(&detect.prefix) {
                out.push(CompletionItem {
                    label,
                    kind: CompletionKind::Reference,
                    detail: Some(value_hint(val)),
                });
            }
        }
    }
    // Primitives have no children — `out` stays empty and the
    // dispatcher closes the popup on its own.

    // Numeric labels (array indices) sort numerically so `9` comes
    // before `10`; mixed / text labels fall back to alpha.
    out.sort_by(
        |a, b| match (a.label.parse::<usize>(), b.label.parse::<usize>()) {
            (Ok(n), Ok(m)) => n.cmp(&m),
            _ => a.label.cmp(&b.label),
        },
    );
    out.dedup_by(|a, b| a.label == b.label);
    out
}

/// Compact one-liner describing a JSON value's shape — used as the
/// `detail` field for ref completion items so the popup shows
/// `Array(12)`, `{3 keys}`, `"select"`, `42`, etc. Mirrors the
/// strings shown in the desktop popup; long strings get truncated
/// so a row-text column doesn't blow up the popup width.
fn value_hint(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "null".into(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => {
            if s.chars().count() > 40 {
                let trimmed: String = s.chars().take(37).collect();
                format!("\"{trimmed}...\"")
            } else {
                format!("\"{s}\"")
            }
        }
        serde_json::Value::Array(a) => format!("Array({})", a.len()),
        serde_json::Value::Object(o) => {
            if o.len() == 1 {
                "{1 key}".into()
            } else {
                format!("{{{} keys}}", o.len())
            }
        }
    }
}

/// Walk back from `(line, offset)` in `body` and return the start
/// offset of the current "word" (alphanumeric / underscore run) plus
/// the prefix string. Returns `None` when the cursor isn't in a
/// completable position (e.g. just after a non-word char or at line
/// start). The dispatcher uses this to decide whether to open the
/// popup and, when accepting, where to splice the chosen label in.
pub fn prefix_at_cursor(body: &str, line: usize, offset: usize) -> Option<(usize, String)> {
    let line_text = body.lines().nth(line)?;
    if offset > line_text.chars().count() {
        return None;
    }
    let chars: Vec<char> = line_text.chars().collect();
    // Walk backwards while the previous char is a word char. The
    // resulting `start` is where the prefix begins; everything from
    // there to `offset` is what the user has typed for the current
    // token.
    let mut start = offset;
    while start > 0 && is_word_char(chars[start - 1]) {
        start -= 1;
    }
    if start == offset {
        return None;
    }
    let prefix: String = chars[start..offset].iter().collect();
    Some((start, prefix))
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_filters_keywords_by_prefix_case_insensitive() {
        // `sel` should surface SELECT (and only SELECT among ANSI).
        let items = complete(Dialect::Generic, "sel", SqlContext::open_no_scope(), None);
        assert!(items.iter().any(|i| i.label == "SELECT"));
        assert!(items.iter().all(|i| i.label.starts_with("SEL")));
    }

    #[test]
    fn complete_includes_dialect_extras_for_postgres() {
        // Postgres adds RETURNING; generic doesn't.
        let pg = complete(
            Dialect::Postgres,
            "RETUR",
            SqlContext::open_no_scope(),
            None,
        );
        assert!(pg.iter().any(|i| i.label == "RETURNING"));
        let gen = complete(Dialect::Generic, "RETUR", SqlContext::open_no_scope(), None);
        assert!(gen.iter().all(|i| i.label != "RETURNING"));
    }

    #[test]
    fn complete_includes_function_builtins_for_dialect() {
        // `date_t` should match `DATE_TRUNC` on Postgres but not on
        // SQLite (where it's not a standard function).
        let pg = complete(
            Dialect::Postgres,
            "date_t",
            SqlContext::open_no_scope(),
            None,
        );
        assert!(pg.iter().any(|i| i.label == "DATE_TRUNC"));
        let sqlite = complete(Dialect::Sqlite, "date_t", SqlContext::open_no_scope(), None);
        assert!(sqlite.iter().all(|i| i.label != "DATE_TRUNC"));
    }

    #[test]
    fn complete_sorts_alphabetically() {
        // Sorted output makes the popup feel predictable across
        // keystrokes — the same prefix always produces the same
        // visual ordering.
        let items = complete(Dialect::Postgres, "co", SqlContext::open_no_scope(), None);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        let mut sorted = labels.clone();
        sorted.sort_unstable();
        assert_eq!(labels, sorted);
    }

    #[test]
    fn complete_empty_prefix_returns_all_candidates_for_dialect() {
        // `<C-Space>` (manual force open) calls with empty prefix —
        // useful for "what's available?". MySQL list should be
        // non-empty and contain its own extras.
        let items = complete(Dialect::MySql, "", SqlContext::open_no_scope(), None);
        assert!(items.iter().any(|i| i.label == "STRAIGHT_JOIN"));
    }

    #[test]
    fn complete_dedups_keyword_function_overlap() {
        // `CASE` shows up as both a keyword and a Postgres function.
        // The popup should list it once, not twice.
        let items = complete(Dialect::Postgres, "CASE", SqlContext::open_no_scope(), None);
        let count = items.iter().filter(|i| i.label == "CASE").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn prefix_at_cursor_returns_word_start_and_chars_typed() {
        // Cursor at the end of `SELE` → prefix is `SELE`, start is 0.
        let body = "SELE";
        let got = prefix_at_cursor(body, 0, 4).expect("has prefix");
        assert_eq!(got.0, 0);
        assert_eq!(got.1, "SELE");
    }

    #[test]
    fn prefix_at_cursor_walks_back_from_mid_word() {
        // `SELECT * FRO` — cursor at end → prefix `FRO`, start at 9.
        let body = "SELECT * FRO";
        let got = prefix_at_cursor(body, 0, 12).expect("has prefix");
        assert_eq!(got.1, "FRO");
        assert_eq!(got.0, 9);
    }

    #[test]
    fn prefix_at_cursor_returns_none_after_non_word_char() {
        // Cursor right after a space → no prefix to complete on.
        let body = "SELECT ";
        assert!(prefix_at_cursor(body, 0, 7).is_none());
    }

    #[test]
    fn prefix_at_cursor_handles_underscore_in_word() {
        // `DATE_TR` includes the underscore as part of the word so
        // mid-word completion still works.
        let body = "DATE_TR";
        let got = prefix_at_cursor(body, 0, 7).expect("has prefix");
        assert_eq!(got.0, 0);
        assert_eq!(got.1, "DATE_TR");
    }

    #[test]
    fn prefix_at_cursor_works_on_second_line() {
        // Multi-line bodies: line 1 starts after the first newline.
        let body = "SELECT *\nFROM us";
        let got = prefix_at_cursor(body, 1, 7).expect("has prefix");
        assert_eq!(got.1, "us");
    }

    // ───────────── SqlContext detection ─────────────
    //
    // The detector handles the four explicit table-naming positions
    // and the `<table>.` column-naming shape. Anything else returns
    // `Open` so we fall back to keywords + builtins.

    #[test]
    fn detect_context_after_from_returns_table() {
        // Cursor right after `FROM ` (anchor_offset=5, line=0).
        // Body left of anchor is `FROM ` → trim → `FROM` → Table.
        let ctx = detect_context("FROM ", 0, 5);
        assert_eq!(ctx, SqlContext::Table);
    }

    #[test]
    fn detect_context_mid_word_after_from_returns_table() {
        // `SELECT * FROM us|` → anchor at start of `us`. The walk
        // sees `SELECT * FROM ` left of the prefix and lands on
        // `FROM` as the last word.
        let body = "SELECT * FROM us";
        let ctx = detect_context(body, 0, 14); // `u` starts at col 14
        assert_eq!(ctx, SqlContext::Table);
    }

    #[test]
    fn detect_context_after_join_returns_table() {
        // `... JOIN orders` mid-word. Same shape as FROM.
        let ctx = detect_context("SELECT * FROM users JOIN o", 0, 25);
        assert_eq!(ctx, SqlContext::Table);
    }

    #[test]
    fn detect_context_after_into_returns_table() {
        // `INSERT INTO ⌷` — `INTO` is the trigger (the `INSERT`
        // word ahead of it doesn't matter for V1).
        let ctx = detect_context("INSERT INTO ", 0, 12);
        assert_eq!(ctx, SqlContext::Table);
    }

    #[test]
    fn detect_context_after_update_returns_table() {
        // `UPDATE ⌷` — table name slot.
        let ctx = detect_context("UPDATE ", 0, 7);
        assert_eq!(ctx, SqlContext::Table);
    }

    #[test]
    fn detect_context_after_word_dot_returns_column_of_word() {
        // `users.|` cursor right after the dot (no prefix yet).
        // `<word>.` is the explicit column-of pattern.
        let ctx = detect_context("SELECT users.", 0, 13);
        assert_eq!(ctx, SqlContext::ColumnOf("users".into()));
    }

    #[test]
    fn detect_context_word_dot_with_partial_column() {
        // `users.id|` — anchor at start of `id`; the `users.` left
        // of it triggers ColumnOf.
        let ctx = detect_context("SELECT users.id", 0, 13);
        assert_eq!(ctx, SqlContext::ColumnOf("users".into()));
    }

    #[test]
    fn detect_context_random_word_returns_open() {
        // `SELECT col` — anchor at start of `col`. Last word before
        // the prefix is `SELECT` (not a table-trigger), so Open.
        let ctx = detect_context("SELECT col", 0, 7);
        assert_eq!(ctx, SqlContext::open_no_scope());
    }

    #[test]
    fn detect_context_at_line_start_returns_open() {
        // No body left of cursor — nothing to trigger on.
        let ctx = detect_context("", 0, 0);
        assert_eq!(ctx, SqlContext::open_no_scope());
    }

    // ───────────── Schema source (Table / ColumnOf) ─────────────

    fn fake_schema() -> Vec<SchemaTable> {
        use crate::schema::SchemaColumn;
        vec![
            SchemaTable {
                schema: Some("public".into()),
                name: "users".into(),
                columns: vec![
                    SchemaColumn {
                        name: "id".into(),
                        data_type: Some("int4".into()),
                    },
                    SchemaColumn {
                        name: "email".into(),
                        data_type: Some("text".into()),
                    },
                    SchemaColumn {
                        name: "name".into(),
                        data_type: Some("text".into()),
                    },
                ],
            },
            SchemaTable {
                schema: Some("public".into()),
                name: "orders".into(),
                columns: vec![
                    SchemaColumn {
                        name: "id".into(),
                        data_type: Some("int4".into()),
                    },
                    SchemaColumn {
                        name: "user_id".into(),
                        data_type: Some("int4".into()),
                    },
                ],
            },
        ]
    }

    #[test]
    fn complete_table_context_surfaces_schema_tables() {
        // Table context + schema cached → tables matching prefix
        // appear, alongside keywords/builtins (a `SELECT` subquery
        // is legal here too, so we keep the keywords).
        let schema = fake_schema();
        let items = complete(Dialect::Postgres, "us", SqlContext::Table, Some(&schema));
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"users"), "users should be in: {labels:?}");
        // Detail carries the schema name so the popup can show
        // `users  (public)` later.
        let users_item = items.iter().find(|i| i.label == "users").unwrap();
        assert_eq!(users_item.kind, CompletionKind::Table);
        assert_eq!(users_item.detail.as_deref(), Some("public"));
    }

    #[test]
    fn complete_column_of_context_surfaces_only_columns() {
        // ColumnOf(users) — popup should list users' columns and
        // *no* keywords. `<users>.SELECT` doesn't make sense.
        let schema = fake_schema();
        let items = complete(
            Dialect::Postgres,
            "",
            SqlContext::ColumnOf("users".into()),
            Some(&schema),
        );
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(labels, vec!["email", "id", "name"]);
        assert!(items.iter().all(|i| i.kind == CompletionKind::Column));
    }

    #[test]
    fn complete_column_of_unknown_table_returns_empty() {
        // ColumnOf(nope) — table not in schema → no items at all
        // (and keywords stay suppressed by the column branch).
        let schema = fake_schema();
        let items = complete(
            Dialect::Postgres,
            "",
            SqlContext::ColumnOf("nope".into()),
            Some(&schema),
        );
        assert!(items.is_empty(), "got: {items:?}");
    }

    #[test]
    fn complete_column_of_table_name_match_is_case_insensitive() {
        // User wrote `Users.|` but the schema has `users`. V1 still
        // matches — case folding is friendlier than failing silently.
        let schema = fake_schema();
        let items = complete(
            Dialect::Postgres,
            "em",
            SqlContext::ColumnOf("Users".into()),
            Some(&schema),
        );
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "email");
    }

    #[test]
    fn complete_table_context_with_no_schema_falls_back_to_keywords() {
        // Schema not yet cached (`None`) → no schema items, but
        // keywords/builtins still appear so the popup isn't empty.
        let items = complete(Dialect::Postgres, "SEL", SqlContext::Table, None);
        assert!(items.iter().any(|i| i.label == "SELECT"));
    }

    // ───────────── Scope extraction + bare column completion ─────────────

    #[test]
    fn extract_tables_in_scope_picks_up_from_clause() {
        // Single-table SELECT — `users` is the only table in scope.
        let scope = extract_tables_in_scope("SELECT id FROM users WHERE id = 1");
        assert_eq!(scope, vec!["users"]);
    }

    #[test]
    fn extract_tables_in_scope_picks_up_join_clauses() {
        // `JOIN` adds tables alongside the FROM target. Order
        // mirrors source order — useful when ranking suggestions.
        let scope =
            extract_tables_in_scope("SELECT * FROM users JOIN orders ON users.id = orders.user_id");
        assert_eq!(scope, vec!["users", "orders"]);
    }

    #[test]
    fn extract_tables_in_scope_dedups_repeats() {
        // The same table joined twice (with aliases) shouldn't
        // double-list — V1 stops at the table name.
        let scope = extract_tables_in_scope("FROM users JOIN users AS u2 ON 1=1");
        assert_eq!(scope, vec!["users"]);
    }

    #[test]
    fn extract_tables_in_scope_skips_subquery_marker() {
        // `FROM (SELECT ...)` — `SELECT` is one of the pseudo-keywords
        // we explicitly skip. The inner `FROM users` still hits.
        let scope = extract_tables_in_scope("SELECT * FROM (SELECT id FROM users) sub");
        assert_eq!(scope, vec!["users"]);
    }

    #[test]
    fn extract_tables_in_scope_returns_empty_for_no_from() {
        // Just `SELECT 1` — no FROM, so no tables in scope.
        assert!(extract_tables_in_scope("SELECT 1").is_empty());
    }

    #[test]
    fn detect_context_after_where_returns_open_with_scope() {
        // `SELECT * FROM users WHERE i⌷` — the cursor sits after
        // a non-trigger word (`WHERE`), so the detector returns
        // Open. The scope must carry `users` so the engine can
        // surface that table's columns.
        let body = "SELECT * FROM users WHERE i";
        let ctx = detect_context(body, 0, 26);
        assert_eq!(
            ctx,
            SqlContext::Open {
                in_scope: vec!["users".into()]
            }
        );
    }

    #[test]
    fn complete_open_with_scope_surfaces_columns_alongside_keywords() {
        // The headline scenario from the screenshot: user has typed
        // `WHERE i` after a known table; the popup must include
        // column names (id) AND the keyword starting with `i` (IF/IN).
        let schema = fake_schema();
        let ctx = SqlContext::Open {
            in_scope: vec!["users".into()],
        };
        let items = complete(Dialect::Postgres, "i", ctx, Some(&schema));
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"id"), "missing column id in {labels:?}");
        assert!(labels.contains(&"IF"), "missing keyword IF in {labels:?}");
    }

    #[test]
    fn complete_open_with_multi_table_scope_keeps_first_match_per_label() {
        // Two tables, both with an `id` column. Dedup-by-label
        // keeps the first occurrence; with `users` listed first in
        // scope, we expect `from users` to win. Disambiguation by
        // explicit `orders.id` still works through ColumnOf.
        let schema = fake_schema();
        let ctx = SqlContext::Open {
            in_scope: vec!["users".into(), "orders".into()],
        };
        let items = complete(Dialect::Postgres, "id", ctx, Some(&schema));
        let id_items: Vec<&CompletionItem> = items.iter().filter(|i| i.label == "id").collect();
        assert_eq!(id_items.len(), 1);
        assert_eq!(id_items[0].detail.as_deref(), Some("from users"));
    }

    #[test]
    fn complete_open_with_scope_but_no_schema_falls_back_to_keywords() {
        // Schema not yet cached → in_scope is meaningless. Engine
        // still produces keywords/builtins so the popup isn't empty.
        let ctx = SqlContext::Open {
            in_scope: vec!["users".into()],
        };
        let items = complete(Dialect::Postgres, "SEL", ctx, None);
        assert!(items.iter().any(|i| i.label == "SELECT"));
    }

    #[test]
    fn complete_table_context_keeps_keywords_alongside_tables() {
        // Verifies keywords keep showing up under Table ctx — a
        // user might be starting a subquery (`FROM (SELECT ...)`).
        let schema = fake_schema();
        let items = complete(Dialect::Postgres, "S", SqlContext::Table, Some(&schema));
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"SELECT"));
    }

    // ───────────── detect_ref_context ──────────────────────────
    //
    // Switching to ref mode is gated on "cursor sits inside an open
    // `{{...}}`". The detector runs before SQL completion so a user
    // mid-ref doesn't get keyword suggestions on top of alias names.

    #[test]
    fn detect_ref_context_returns_none_when_no_open_brace() {
        // Plain SQL with no `{{` to the left of the cursor.
        assert!(detect_ref_context("SELECT * FROM users", 0, 19).is_none());
    }

    #[test]
    fn detect_ref_context_picks_up_empty_open() {
        // `{{|` — cursor right after the opener. Prefix is empty,
        // path is None, anchor sits at the cursor.
        let got = detect_ref_context("SELECT * WHERE x = {{", 0, 21).unwrap();
        assert_eq!(got.prefix, "");
        assert_eq!(got.path, None);
        assert_eq!(got.anchor_offset, 21);
    }

    #[test]
    fn detect_ref_context_returns_prefix_for_first_segment() {
        // `{{q1|` — typing the alias. Prefix is what's typed since
        // the opener; no `.` yet so path is None.
        let got = detect_ref_context("SELECT {{q1", 0, 11).unwrap();
        assert_eq!(got.prefix, "q1");
        assert_eq!(got.path, None);
        assert_eq!(got.anchor_offset, 9); // start of `q1`
    }

    #[test]
    fn detect_ref_context_splits_path_after_dot() {
        // `{{q1.r|` — the path holds the alias, prefix is the new
        // segment ("r"). Lets the engine pivot from "list aliases"
        // to "list keys of q1's cached result".
        let got = detect_ref_context("SELECT {{q1.r", 0, 13).unwrap();
        assert_eq!(got.prefix, "r");
        assert_eq!(got.path.as_deref(), Some("q1"));
        assert_eq!(got.anchor_offset, 12); // start of `r`
    }

    #[test]
    fn detect_ref_context_supports_multi_segment_path() {
        // `{{q1.response.|` — two-segment path, empty current
        // prefix. Engine walks into `cached_result.response.*`.
        let got = detect_ref_context("SELECT {{q1.response.", 0, 21).unwrap();
        assert_eq!(got.prefix, "");
        assert_eq!(got.path.as_deref(), Some("q1.response"));
    }

    #[test]
    fn detect_ref_context_returns_none_when_brace_already_closed() {
        // `{{q1}} AND id = 5|` — cursor is past a closed ref.
        assert!(
            detect_ref_context("WHERE x = {{q1}} AND id = 5", 0, 27).is_none(),
            "closed ref should not trigger"
        );
    }

    // ───────────── complete_refs ──────────────────────────

    fn make_ref_doc(md: &str) -> crate::buffer::Document {
        crate::buffer::Document::from_markdown(md).expect("parse")
    }

    fn refs_doc_blocks(doc: &crate::buffer::Document) -> Vec<usize> {
        doc.segments()
            .iter()
            .enumerate()
            .filter_map(|(i, s)| matches!(s, crate::buffer::Segment::Block(_)).then_some(i))
            .collect()
    }

    #[test]
    fn complete_refs_no_path_lists_aliases_and_envs() {
        // Top-level: aliases first (alphabetical), env vars after,
        // both filtered by prefix and tagged with kind=Reference.
        let md = "```db-postgres alias=q1\nSELECT 1\n```\n\n```db-postgres alias=q2\nSELECT 2\n```\n\n```db-postgres alias=cur\nSELECT 3\n```\n";
        let doc = make_ref_doc(md);
        let blocks = refs_doc_blocks(&doc);
        let mut envs = std::collections::HashMap::new();
        envs.insert("API_TOKEN".to_string(), "abc".to_string());
        let detect = RefDetect {
            anchor_offset: 0,
            prefix: "q".to_string(),
            path: None,
        };
        let items = complete_refs(&detect, doc.segments(), blocks[2], &envs);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(labels, vec!["q1", "q2"]);
        assert!(items.iter().all(|i| i.kind == CompletionKind::Reference));
    }

    #[test]
    fn complete_refs_no_path_includes_env_vars() {
        // Env keys also surface — popular for `{{API_KEY}}`-style refs.
        let md = "```db-postgres alias=cur\nSELECT 1\n```\n";
        let doc = make_ref_doc(md);
        let blocks = refs_doc_blocks(&doc);
        let mut envs = std::collections::HashMap::new();
        envs.insert("API_TOKEN".to_string(), "abc".to_string());
        let detect = RefDetect {
            anchor_offset: 0,
            prefix: "API".to_string(),
            path: None,
        };
        let items = complete_refs(&detect, doc.segments(), blocks[0], &envs);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(labels, vec!["API_TOKEN"]);
        assert_eq!(items[0].detail.as_deref(), Some("env"));
    }

    /// Helper: build a fully-fleshed `cached_result` matching what
    /// `handle_db_block_result` would store after a SELECT — used
    /// across the JSON-walk tests below.
    fn fake_db_cache() -> serde_json::Value {
        serde_json::json!({
            "results": [
                {
                    "kind": "select",
                    "columns": [
                        { "name": "id", "type": "int4" },
                        { "name": "name", "type": "text" }
                    ],
                    "rows": [
                        { "id": 7, "name": "alice" },
                        { "id": 8, "name": "bob" }
                    ],
                    "has_more": false
                }
            ],
            "messages": [],
            "stats": { "elapsed_ms": 12 }
        })
    }

    fn doc_with_cached_q1(md: &str) -> (crate::buffer::Document, Vec<usize>) {
        let mut doc = make_ref_doc(md);
        let blocks = refs_doc_blocks(&doc);
        if let Some(b) = doc.block_at_mut(blocks[0]) {
            b.cached_result = Some(fake_db_cache());
            b.state = crate::buffer::block::ExecutionState::Success;
        }
        (doc, blocks)
    }

    const TWO_BLOCK_MD: &str =
        "```db-postgres alias=q1\nSELECT 1\n```\n\n```db-postgres alias=cur\nSELECT 1\n```\n";

    #[test]
    fn complete_refs_alias_only_lists_synthetic_envelope() {
        // `{{q1.|}}` — the synthetic root that the autocomplete
        // walks against is `{response, status}`, *not* the raw
        // cached_result. Matches desktop's `references.ts:140-143`
        // behavior: top-level shows `response` and `status`, the
        // legacy first-row column shim is NOT exposed here (it's
        // only a runtime resolver shim, see `resolve_one_ref`).
        let (doc, blocks) = doc_with_cached_q1(TWO_BLOCK_MD);
        let detect = RefDetect {
            anchor_offset: 0,
            prefix: "".to_string(),
            path: Some("q1".to_string()),
        };
        let items = complete_refs(
            &detect,
            doc.segments(),
            blocks[1],
            &std::collections::HashMap::new(),
        );
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(labels, vec!["response", "status"]);
        // Detail carries the value-shape hint, like the desktop popup.
        let response_item = items.iter().find(|i| i.label == "response").unwrap();
        assert_eq!(response_item.detail.as_deref(), Some("{3 keys}"));
        let status_item = items.iter().find(|i| i.label == "status").unwrap();
        assert_eq!(status_item.detail.as_deref(), Some("\"success\""));
    }

    #[test]
    fn complete_refs_path_response_lists_db_top_keys() {
        // `{{q1.response.|}}` — now we're inside the actual cached
        // response, so popup shows its real top-level keys
        // (results/messages/stats) with shape hints.
        let (doc, blocks) = doc_with_cached_q1(TWO_BLOCK_MD);
        let detect = RefDetect {
            anchor_offset: 0,
            prefix: "".to_string(),
            path: Some("q1.response".to_string()),
        };
        let items = complete_refs(
            &detect,
            doc.segments(),
            blocks[1],
            &std::collections::HashMap::new(),
        );
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"results"), "got: {labels:?}");
        assert!(labels.contains(&"messages"), "got: {labels:?}");
        assert!(labels.contains(&"stats"), "got: {labels:?}");
        // No legacy `first row` shim anymore — `id`/`name` should
        // NOT appear at this level.
        assert!(!labels.contains(&"id"));
        assert!(!labels.contains(&"name"));
    }

    #[test]
    fn complete_refs_path_response_results_lists_array_indices() {
        // `{{q1.response.results.|}}` — popup walks into the array
        // and lists its numeric indices (just `0` for a single
        // result set). Each carries a `{N keys}` detail hint.
        let (doc, blocks) = doc_with_cached_q1(TWO_BLOCK_MD);
        let detect = RefDetect {
            anchor_offset: 0,
            prefix: "".to_string(),
            path: Some("q1.response.results".to_string()),
        };
        let items = complete_refs(
            &detect,
            doc.segments(),
            blocks[1],
            &std::collections::HashMap::new(),
        );
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(labels, vec!["0"]);
        let zero = items.iter().find(|i| i.label == "0").unwrap();
        assert!(zero
            .detail
            .as_deref()
            .map(|d| d.contains("keys"))
            .unwrap_or(false));
    }

    #[test]
    fn complete_refs_path_response_results_0_lists_result_keys() {
        // `{{q1.response.results.0.|}}` — popup lists the keys of
        // that result set (matches the desktop screenshot:
        // `columns`, `has_more`, `kind`, `rows`).
        let (doc, blocks) = doc_with_cached_q1(TWO_BLOCK_MD);
        let detect = RefDetect {
            anchor_offset: 0,
            prefix: "".to_string(),
            path: Some("q1.response.results.0".to_string()),
        };
        let items = complete_refs(
            &detect,
            doc.segments(),
            blocks[1],
            &std::collections::HashMap::new(),
        );
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"columns"));
        assert!(labels.contains(&"rows"));
        assert!(labels.contains(&"kind"));
        assert!(labels.contains(&"has_more"));
    }

    #[test]
    fn complete_refs_path_into_primitive_returns_empty() {
        // `{{q1.status.|}}` — `status` is a string primitive, no
        // children. Popup is empty; dispatcher closes it.
        let (doc, blocks) = doc_with_cached_q1(TWO_BLOCK_MD);
        let detect = RefDetect {
            anchor_offset: 0,
            prefix: "".to_string(),
            path: Some("q1.status".to_string()),
        };
        let items = complete_refs(
            &detect,
            doc.segments(),
            blocks[1],
            &std::collections::HashMap::new(),
        );
        assert!(items.is_empty(), "got: {items:?}");
    }

    #[test]
    fn complete_refs_path_to_unknown_alias_returns_empty() {
        // `{{ghost.|}}` — alias doesn't exist above. Popup empty
        // (caller closes it) so user sees nothing instead of a
        // confusing wrong list.
        let md = "```db-postgres alias=cur\nSELECT 1\n```\n";
        let doc = make_ref_doc(md);
        let blocks = refs_doc_blocks(&doc);
        let detect = RefDetect {
            anchor_offset: 0,
            prefix: "".to_string(),
            path: Some("ghost".to_string()),
        };
        let items = complete_refs(
            &detect,
            doc.segments(),
            blocks[0],
            &std::collections::HashMap::new(),
        );
        assert!(items.is_empty());
    }

    #[test]
    fn complete_refs_skips_blocks_at_or_below_current() {
        // Refs can only point to blocks ABOVE the current one. A
        // block sitting after `cur` mustn't appear in the popup.
        let md = "```db-postgres alias=cur\nSELECT 1\n```\n\n```db-postgres alias=below\nSELECT 2\n```\n";
        let doc = make_ref_doc(md);
        let blocks = refs_doc_blocks(&doc);
        let detect = RefDetect {
            anchor_offset: 0,
            prefix: "".to_string(),
            path: None,
        };
        let items = complete_refs(
            &detect,
            doc.segments(),
            blocks[0], // `cur` is the first block
            &std::collections::HashMap::new(),
        );
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(!labels.contains(&"below"));
        assert!(!labels.contains(&"cur"));
    }

    #[test]
    fn dialect_from_block_maps_known_types() {
        // `db-postgres` → Postgres; unknown → Generic. Smoke test
        // for the reverse mapping the dispatcher uses.
        use crate::buffer::block::{BlockId, ExecutionState};
        let mk = |ty: &str| BlockNode {
            id: BlockId(0),
            raw: ropey::Rope::new(),
            block_type: ty.to_string(),
            alias: None,
            display_mode: None,
            params: serde_json::json!({}),
            state: ExecutionState::Idle,
            cached_result: None,
        };
        assert_eq!(Dialect::from_block(&mk("db-postgres")), Dialect::Postgres);
        assert_eq!(Dialect::from_block(&mk("db-mysql")), Dialect::MySql);
        assert_eq!(Dialect::from_block(&mk("db-sqlite")), Dialect::Sqlite);
        assert_eq!(Dialect::from_block(&mk("http")), Dialect::Generic);
    }

    // ───────────── explain_wrap ──────────────────────────

    #[test]
    fn explain_wrap_postgres_uses_bare_explain() {
        let got = explain_wrap("SELECT * FROM users", Dialect::Postgres);
        assert_eq!(got, "EXPLAIN SELECT * FROM users");
    }

    #[test]
    fn explain_wrap_mysql_uses_bare_explain() {
        let got = explain_wrap("SELECT id FROM users", Dialect::MySql);
        assert_eq!(got, "EXPLAIN SELECT id FROM users");
    }

    #[test]
    fn explain_wrap_sqlite_uses_explain_query_plan() {
        // SQLite's plain EXPLAIN dumps VDBE bytecode; QUERY PLAN
        // is the human-readable variant — match desktop's choice.
        let got = explain_wrap("SELECT * FROM users", Dialect::Sqlite);
        assert_eq!(got, "EXPLAIN QUERY PLAN SELECT * FROM users");
    }

    #[test]
    fn explain_wrap_strips_trailing_semicolon() {
        // Otherwise the executor's `;`-splitter would see an empty
        // second statement and emit a confusing extra result.
        let got = explain_wrap("SELECT 1;", Dialect::Postgres);
        assert_eq!(got, "EXPLAIN SELECT 1");
    }

    #[test]
    fn explain_wrap_takes_only_first_statement() {
        // V1 only explains the first statement when the user wrote
        // multi-statement. Explaining each individually is V2.
        let got = explain_wrap("SELECT 1; SELECT 2", Dialect::Postgres);
        assert_eq!(got, "EXPLAIN SELECT 1");
    }
}
