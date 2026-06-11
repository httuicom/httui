use super::*;
use crate::buffer::block::BlockId;
use serde_json::json;

fn db_block(result: Option<serde_json::Value>) -> BlockNode {
    BlockNode {
        id: BlockId(0),
        raw: ropey::Rope::new(),
        block_type: "db-sqlite".into(),
        alias: None,
        display_mode: None,
        params: json!({"query": "SELECT 1"}),
        state: if result.is_some() {
            ExecutionState::Success
        } else {
            ExecutionState::Idle
        },
        cached_result: result,
    }
}

#[test]
fn db_summary_select_with_count_and_elapsed() {
    let b = db_block(Some(json!({
        "stats": {"elapsed_ms": 12},
        "results": [{"kind": "select", "rows": [{"a": 1}, {"a": 2}], "has_more": false}],
    })));
    let s = db_summary(&b).unwrap();
    assert!(s.contains("2 rows"));
    assert!(s.contains("12ms"));
}

#[test]
fn db_summary_select_has_more_appends_plus() {
    let b = db_block(Some(json!({
        "stats": {"elapsed_ms": 1},
        "results": [{"kind": "select", "rows": [{"a": 1}], "has_more": true}],
    })));
    let s = db_summary(&b).unwrap();
    assert!(s.contains("1+"));
}

#[test]
fn db_summary_mutation_format() {
    let b = db_block(Some(json!({
        "stats": {"elapsed_ms": 4},
        "results": [{"kind": "mutation", "rows_affected": 3}],
    })));
    let s = db_summary(&b).unwrap();
    assert!(s.contains("3 affected"));
}

#[test]
fn db_summary_error_includes_position_suffix() {
    let b = db_block(Some(json!({
        "stats": {"elapsed_ms": 1},
        "results": [{"kind": "error", "message": "bad sql", "line": 2, "column": 5}],
    })));
    let s = db_summary(&b).unwrap();
    assert!(s.contains("bad sql"));
    assert!(s.contains("at 2:5"));
}

#[test]
fn db_summary_multi_result_appends_more_suffix() {
    let b = db_block(Some(json!({
        "stats": {"elapsed_ms": 1},
        "results": [
            {"kind": "select", "rows": [], "has_more": false},
            {"kind": "select", "rows": [], "has_more": false},
        ],
    })));
    let s = db_summary(&b).unwrap();
    assert!(s.contains("(+1 more)"));
}

#[test]
fn db_summary_returns_none_for_unknown_kind() {
    let b = db_block(Some(json!({
        "stats": {"elapsed_ms": 1},
        "results": [{"kind": "wat", "rows": []}],
    })));
    assert!(db_summary(&b).is_none());
}

#[test]
fn error_position_extracts_line_and_column() {
    let b = db_block(Some(json!({
        "results": [{"kind": "error", "line": 7, "column": 3}],
    })));
    assert_eq!(error_position(&b), Some((7, 3)));
}

#[test]
fn error_position_defaults_column_when_missing() {
    let b = db_block(Some(json!({
        "results": [{"kind": "error", "line": 4}],
    })));
    assert_eq!(error_position(&b), Some((4, 1)));
}

#[test]
fn error_position_returns_none_for_select_result() {
    let b = db_block(Some(json!({
        "results": [{"kind": "select", "rows": []}],
    })));
    assert!(error_position(&b).is_none());
}

#[test]
fn clamp_viewport_no_scroll_when_total_fits_window() {
    assert_eq!(clamp_result_viewport(0, 10, 4, 5), 0);
}

#[test]
fn clamp_viewport_scrolls_down_to_keep_cursor_visible() {
    assert_eq!(clamp_result_viewport(0, 10, 25, 80), 18);
}

#[test]
fn clamp_viewport_scrolls_up_to_keep_cursor_visible() {
    assert_eq!(clamp_result_viewport(20, 10, 5, 80), 3);
}

#[test]
fn clamp_viewport_zero_returns_zero() {
    assert_eq!(clamp_result_viewport(7, 0, 50, 100), 0);
}

#[test]
fn build_result_table_none_without_cache() {
    let b = db_block(None);
    assert!(build_result_table(&b, None, None, MAX_VISIBLE_ROWS).is_none());
}

#[test]
fn build_result_table_none_for_mutation_kind() {
    let b = db_block(Some(json!({
        "stats": {"elapsed_ms": 1},
        "results": [{"kind": "mutation", "rows_affected": 3}],
    })));
    assert!(build_result_table(&b, None, None, MAX_VISIBLE_ROWS).is_none());
}

#[test]
fn build_result_table_some_for_select_with_columns() {
    let b = db_block(Some(json!({
        "stats": {"elapsed_ms": 1},
        "results": [{
            "kind": "select",
            "columns": [{"name": "id", "type": "int"}],
            "rows": [{"id": 1}, {"id": 2}],
            "has_more": false,
        }],
    })));
    let (_, sel) = build_result_table(&b, Some(1), None, MAX_VISIBLE_ROWS).unwrap();
    assert_eq!(sel, Some(1));
}

fn select_block_with_rows(n: usize) -> BlockNode {
    let rows: Vec<serde_json::Value> = (0..n).map(|i| json!({"id": i})).collect();
    db_block(Some(json!({
        "stats": {"elapsed_ms": 1},
        "results": [{
            "kind": "select",
            "columns": [{"name": "id", "type": "int"}],
            "rows": rows,
            "has_more": false,
        }],
    })))
}

#[test]
fn build_result_table_persistent_viewport_slides() {
    let b = select_block_with_rows(30);
    let mut vt: u16 = 0;
    build_result_table(&b, Some(15), Some(&mut vt), MAX_VISIBLE_ROWS);
    assert_eq!(vt, 8);
}

#[test]
fn build_result_table_fills_a_taller_window() {
    // A 25-row budget shows all 25 rows of a 30-row result at
    // once (the old fixed cap would clip at 10).
    let b = select_block_with_rows(30);
    let mut vt: u16 = 0;
    let (table, _) = build_result_table(&b, Some(0), Some(&mut vt), 25).unwrap();
    // Window did not need to slide for the first row.
    assert_eq!(vt, 0);
    // Visible slice covers max_rows: cursor at the end forces the
    // window to start at total - max_rows.
    build_result_table(&b, Some(29), Some(&mut vt), 25).unwrap();
    assert_eq!(vt as usize, 30 - 25);
    drop(table);
}

#[test]
fn build_result_table_zero_budget_still_shows_one_row() {
    let b = select_block_with_rows(5);
    let mut vt: u16 = 0;
    assert!(build_result_table(&b, Some(0), Some(&mut vt), 0).is_some());
}

#[test]
fn clamp_viewport_adapts_to_a_larger_window() {
    // viewport 25, cursor at the last of 80 rows → window pinned
    // to the tail (80 - 25 = 55).
    assert_eq!(clamp_result_viewport(0, 25, 79, 80), 55);
    // Everything fits → no scroll.
    assert_eq!(clamp_result_viewport(3, 25, 10, 20), 0);
}

#[test]
fn db_result_table_height_zero_when_no_cache() {
    let b = db_block(None);
    assert_eq!(db_result_table_height(&b), 0);
}

#[test]
fn db_result_table_height_for_select_with_rows() {
    let b = db_block(Some(json!({
        "results": [{
            "kind": "select",
            "columns": [{"name": "id", "type": "int"}],
            "rows": [{"id": 1}, {"id": 2}, {"id": 3}],
            "has_more": false,
        }],
    })));
    // header (1) + 3 rows + tab bar + separator = 6
    assert_eq!(db_result_table_height(&b), 6);
}

#[test]
fn db_result_table_height_caps_at_viewport_for_huge_result() {
    let rows: Vec<serde_json::Value> = (0..40).map(|i| json!({"id": i})).collect();
    let b = db_block(Some(json!({
        "results": [{
            "kind": "select",
            "columns": [{"name": "id", "type": "int"}],
            "rows": rows,
            "has_more": false,
        }],
    })));
    assert_eq!(
        db_result_table_height(&b),
        (1 + MAX_VISIBLE_ROWS + 2) as u16
    );
}

#[test]
fn db_result_table_height_error_kind_gets_fixed_panel() {
    let b = db_block(Some(json!({
        "results": [{"kind": "error", "message": "x"}],
    })));
    // ERROR_PANEL_ROWS (6) + 2 chrome rows = 8
    assert_eq!(db_result_table_height(&b), 8);
}

#[test]
fn is_numeric_type_matches_common_sql_types() {
    for t in &[
        "int", "INTEGER", "bigint", "float", "real", "decimal", "numeric", "money", "int4",
        "FLOAT8",
    ] {
        assert!(is_numeric_type(t), "expected {t} numeric");
    }
    assert!(!is_numeric_type("text"));
    assert!(!is_numeric_type("varchar"));
}

#[test]
fn truncate_with_ellipsis_passes_short_strings_through() {
    assert_eq!(truncate_with_ellipsis("abc", 5), "abc");
    assert_eq!(truncate_with_ellipsis("abc", 3), "abc");
}

#[test]
fn truncate_with_ellipsis_drops_with_ellipsis_when_over_width() {
    let r = truncate_with_ellipsis("abcdef", 4);
    assert_eq!(r.chars().count(), 4);
    assert!(r.ends_with('…'));
}

#[test]
fn truncate_with_ellipsis_zero_width_yields_empty() {
    assert_eq!(truncate_with_ellipsis("abc", 0), "");
}

#[test]
fn format_cell_translates_each_json_kind() {
    use serde_json::json;
    assert_eq!(format_cell(&json!(null)), "(null)");
    assert_eq!(format_cell(&json!(true)), "true");
    assert_eq!(format_cell(&json!(42)), "42");
    assert_eq!(format_cell(&json!("hi")), "hi");
    assert_eq!(format_cell(&json!([1, 2])), "[…]");
    assert_eq!(format_cell(&json!({"k": 1})), "{…}");
}
