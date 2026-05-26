// size:exclude file — DB commands test suite (cluster of small unit tests).

use super::*;
use crate::buffer::Document;

// ───────────── doc / cache fixtures ─────────────

fn make_doc(md: &str) -> Document {
    Document::from_markdown(md).expect("valid markdown")
}

fn set_cache(doc: &mut Document, idx: usize, v: serde_json::Value) {
    let block = doc
        .block_at_mut(idx)
        .expect("segment idx should be a block");
    block.cached_result = Some(v);
}

fn block_indices(doc: &Document) -> Vec<usize> {
    doc.segments()
        .iter()
        .enumerate()
        .filter_map(|(i, s)| matches!(s, Segment::Block(_)).then_some(i))
        .collect()
}

fn empty_env() -> std::collections::HashMap<String, String> {
    std::collections::HashMap::new()
}

fn env_map(pairs: &[(&str, &str)]) -> std::collections::HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn db_response(results: serde_json::Value) -> serde_json::Value {
    // Pre-redesign caches (no `results` array) bypass the shim — see
    // `is_db_response_shape`.
    serde_json::json!({
        "results": results,
        "messages": [],
        "plan": serde_json::Value::Null,
        "stats": { "elapsed_ms": 12 }
    })
}

fn select_result(rows: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "kind": "select",
        "columns": [],
        "rows": rows,
        "has_more": false
    })
}

// ───────────── SQL classifiers ─────────────

#[test]
fn cacheable_query_recognizes_select_family() {
    for q in &[
        "SELECT 1",
        "select 1",
        "  SELECT * FROM foo",
        "WITH x AS (...) SELECT 1",
        "EXPLAIN SELECT 1",
        "PRAGMA table_info('users')",
        "SHOW TABLES",
        "DESC users",
    ] {
        assert!(is_cacheable_query(q), "expected cacheable: {q}");
    }
}

#[test]
fn cacheable_query_rejects_mutations() {
    for q in &[
        "UPDATE users SET x = 1",
        "DELETE FROM users",
        "INSERT INTO users VALUES (1)",
        "REPLACE INTO users VALUES (1)",
        "CREATE TABLE x (id INT)",
        "DROP TABLE x",
        "ALTER TABLE x ADD COLUMN y INT",
        "TRUNCATE TABLE x",
    ] {
        assert!(!is_cacheable_query(q), "expected mutation: {q}");
    }
}

#[test]
fn cacheable_query_strips_leading_comments() {
    assert!(is_cacheable_query("-- daily report\nSELECT 1"));
    assert!(is_cacheable_query("/* multi\n   line */\nSELECT 1"));
    assert!(!is_cacheable_query("-- cleanup job\nDELETE FROM users"));
}

#[test]
fn writing_query_recognizes_mutations() {
    for q in &[
        "UPDATE users SET x=1",
        "DELETE FROM users",
        "INSERT INTO t VALUES (1)",
        "REPLACE INTO t VALUES (1)",
        "MERGE INTO t USING ...",
        "CREATE TABLE x (id INT)",
        "DROP TABLE x",
        "ALTER TABLE x ADD COLUMN y INT",
        "TRUNCATE TABLE x",
        "GRANT SELECT ON t TO u",
        "REVOKE SELECT ON t FROM u",
        "VACUUM",
    ] {
        assert!(is_writing_query(q), "expected write: {q}");
    }
}

#[test]
fn writing_query_rejects_reads() {
    for q in &[
        "SELECT 1",
        "SELECT * FROM users",
        "WITH x AS (SELECT 1) SELECT * FROM x",
        "EXPLAIN SELECT 1",
        "PRAGMA table_info('x')",
        "SHOW TABLES",
        "DESC users",
    ] {
        assert!(!is_writing_query(q), "should not be write: {q}");
    }
}

#[test]
fn unscoped_destructive_flags_update_without_where() {
    assert!(is_unscoped_destructive("UPDATE users SET x = 1"));
    assert!(is_unscoped_destructive("DELETE FROM users"));
    assert!(is_unscoped_destructive("update users set name = 'x'"));
}

#[test]
fn unscoped_destructive_passes_when_where_present() {
    assert!(!is_unscoped_destructive(
        "UPDATE users SET x = 1 WHERE id = 7"
    ));
    assert!(!is_unscoped_destructive(
        "DELETE FROM users WHERE active = 0"
    ));
    assert!(!is_unscoped_destructive("delete from users where id < 10"));
}

#[test]
fn unscoped_destructive_is_word_boundary_aware() {
    // A column literally named `whereabouts` must not be mistaken
    // for the WHERE keyword.
    assert!(is_unscoped_destructive(
        "UPDATE users SET whereabouts = 'home'"
    ));
}

#[test]
fn unscoped_destructive_skips_other_writes() {
    assert!(!is_unscoped_destructive("INSERT INTO users VALUES (1)"));
    assert!(!is_unscoped_destructive("DROP TABLE users"));
    assert!(!is_unscoped_destructive("CREATE TABLE t (id INT)"));
}

#[test]
fn unscoped_destructive_strips_leading_comments() {
    assert!(is_unscoped_destructive(
        "-- run after midnight\nDELETE FROM users"
    ));
    assert!(!is_unscoped_destructive(
        "-- legit\nDELETE FROM users WHERE inactive = 1"
    ));
}

// ───────────── cache hash ─────────────

#[test]
fn cache_hash_is_deterministic_for_same_inputs() {
    let env = env_map(&[("TOKEN", "abc")]);
    let h1 = compute_db_cache_hash("SELECT 1 WHERE x = {{TOKEN}}", Some("conn-1"), &env);
    let h2 = compute_db_cache_hash("SELECT 1 WHERE x = {{TOKEN}}", Some("conn-1"), &env);
    assert_eq!(h1, h2);
}

#[test]
fn cache_hash_changes_when_referenced_env_value_changes() {
    let body = "SELECT 1 WHERE x = {{TOKEN}}";
    let h_old = compute_db_cache_hash(body, Some("conn-1"), &env_map(&[("TOKEN", "old")]));
    let h_new = compute_db_cache_hash(body, Some("conn-1"), &env_map(&[("TOKEN", "new")]));
    assert_ne!(h_old, h_new);
}

#[test]
fn cache_hash_ignores_unreferenced_env_vars() {
    let body = "SELECT 1";
    let h1 = compute_db_cache_hash(body, Some("conn-1"), &env_map(&[]));
    let h2 = compute_db_cache_hash(body, Some("conn-1"), &env_map(&[("UNRELATED", "v")]));
    assert_eq!(h1, h2);
}

#[test]
fn cache_hash_changes_with_connection_id() {
    let body = "SELECT 1";
    let env = env_map(&[]);
    let h1 = compute_db_cache_hash(body, Some("conn-a"), &env);
    let h2 = compute_db_cache_hash(body, Some("conn-b"), &env);
    assert_ne!(h1, h2);
}

// ───────────── db_summary_from_value ─────────────

#[test]
fn db_summary_from_value_handles_select_with_extras() {
    let value = serde_json::json!({
        "results": [
            { "kind": "select", "rows": [{}, {}, {}], "has_more": false },
            { "kind": "select", "rows": [{}], "has_more": false },
        ],
        "stats": { "elapsed_ms": 0 }
    });
    let s = db_summary_from_value(Some(&value), 12);
    assert_eq!(s, "3 rows · 12ms (+1 more)");
}

#[test]
fn db_summary_from_value_describes_mutation() {
    let value = serde_json::json!({
        "results": [{ "kind": "mutation", "rows_affected": 7 }],
        "stats": { "elapsed_ms": 0 }
    });
    let s = db_summary_from_value(Some(&value), 4);
    assert_eq!(s, "7 affected · 4ms");
}

#[test]
fn db_summary_from_value_appends_line_column_for_error() {
    // Postgres returns byte `position`; executor enriches into
    // `(line, column)`. Summary surfaces where the parser tripped.
    let value = serde_json::json!({
        "results": [
            {
                "kind": "error",
                "message": "syntax error at or near \"FORM\"",
                "line": 2,
                "column": 5
            }
        ],
        "stats": { "elapsed_ms": 4 }
    });
    let s = db_summary_from_value(Some(&value), 4);
    assert_eq!(s, "error: syntax error at or near \"FORM\" at 2:5");
}

#[test]
fn db_summary_from_value_omits_position_when_absent() {
    let value = serde_json::json!({
        "results": [
            {
                "kind": "error",
                "message": "connection lost"
            }
        ],
        "stats": { "elapsed_ms": 0 }
    });
    let s = db_summary_from_value(Some(&value), 0);
    assert_eq!(s, "error: connection lost");
}

// ───────────── resolve_block_refs (bind-params) ─────────────
//
// Security invariant: every `{{ref}}` value must leave the function
// as a *bind value* — never as part of the SQL string.

#[test]
fn resolve_block_refs_replaces_refs_with_question_marks() {
    let md = "```http alias=upstream\nGET /users/7\n```\n\n```db-postgres alias=q\nSELECT 1\n```\n";
    let mut doc = make_doc(md);
    let blocks = block_indices(&doc);
    set_cache(&mut doc, blocks[0], serde_json::json!({ "id": 7 }));
    let (sql, binds) = resolve_block_refs(
        doc.segments(),
        blocks[1],
        "SELECT * FROM users WHERE id = {{upstream.id}}",
        &empty_env(),
    )
    .expect("resolves");
    assert_eq!(sql, "SELECT * FROM users WHERE id = ?");
    assert_eq!(binds, vec![serde_json::json!(7)]);
}

#[test]
fn resolve_block_refs_blocks_sql_injection_via_string_value() {
    // Injection payload returned by an upstream block: the
    // single-quote-and-DROP must NOT escape into the SQL string.
    let md = "```http alias=evil\nGET /\n```\n\n```db-postgres alias=q\nSELECT 1\n```\n";
    let mut doc = make_doc(md);
    let blocks = block_indices(&doc);
    let payload = "7'; DROP TABLE users; --";
    set_cache(&mut doc, blocks[0], serde_json::json!({ "id": payload }));
    let (sql, binds) = resolve_block_refs(
        doc.segments(),
        blocks[1],
        "SELECT * FROM users WHERE id = {{evil.id}}",
        &empty_env(),
    )
    .expect("resolves");
    assert_eq!(sql, "SELECT * FROM users WHERE id = ?");
    assert!(
        !sql.contains("DROP"),
        "injection payload leaked into SQL: {sql}"
    );
    assert_eq!(binds, vec![serde_json::Value::String(payload.to_string())]);
}

#[test]
fn resolve_block_refs_emits_one_bind_per_placeholder_in_order() {
    // sqlx slices binds per-statement by `count_placeholders`, so
    // ordering matters for multi-statement.
    let md = "```http alias=src\nGET /\n```\n\n```db-postgres alias=q\nSELECT 1\n```\n";
    let mut doc = make_doc(md);
    let blocks = block_indices(&doc);
    set_cache(
        &mut doc,
        blocks[0],
        serde_json::json!({ "a": 1, "b": "two", "c": true }),
    );
    let (sql, binds) = resolve_block_refs(
        doc.segments(),
        blocks[1],
        "SELECT {{src.a}}, {{src.b}}, {{src.c}}",
        &empty_env(),
    )
    .expect("resolves");
    assert_eq!(sql, "SELECT ?, ?, ?");
    assert_eq!(
        binds,
        vec![
            serde_json::json!(1),
            serde_json::json!("two"),
            serde_json::json!(true),
        ]
    );
}

#[test]
fn resolve_block_refs_preserves_value_types() {
    // Driver decides numeric coercion — earlier code stringified
    // each into a SQL literal.
    let md = "```http alias=src\nGET /\n```\n\n```db-postgres alias=q\nSELECT 1\n```\n";
    let mut doc = make_doc(md);
    let blocks = block_indices(&doc);
    set_cache(
        &mut doc,
        blocks[0],
        serde_json::json!({ "n": 42, "f": false, "z": serde_json::Value::Null }),
    );
    let (_, binds) = resolve_block_refs(
        doc.segments(),
        blocks[1],
        "SELECT {{src.n}}, {{src.f}}, {{src.z}}",
        &empty_env(),
    )
    .expect("resolves");
    assert!(binds[0].is_number(), "number type lost: {:?}", binds[0]);
    assert!(binds[1].is_boolean(), "bool type lost: {:?}", binds[1]);
    assert!(binds[2].is_null(), "null type lost: {:?}", binds[2]);
}

#[test]
fn resolve_block_refs_env_var_becomes_string_bind() {
    let mut env = std::collections::HashMap::new();
    env.insert("API_TOKEN".to_string(), "abc-123".to_string());
    let md = "```db-postgres alias=q\nSELECT 1\n```\n";
    let doc = make_doc(md);
    let blocks = block_indices(&doc);
    let (sql, binds) = resolve_block_refs(doc.segments(), blocks[0], "SELECT {{API_TOKEN}}", &env)
        .expect("resolves");
    assert_eq!(sql, "SELECT ?");
    assert_eq!(binds, vec![serde_json::json!("abc-123")]);
}

#[test]
fn resolve_block_refs_rejects_array_or_object_value() {
    // Drivers can't bind a JSON array/object on the dialects we
    // target — caller sees a clear error instead of a silent
    // stringify.
    let md = "```http alias=src\nGET /\n```\n\n```db-postgres alias=q\nSELECT 1\n```\n";
    let mut doc = make_doc(md);
    let blocks = block_indices(&doc);
    set_cache(
        &mut doc,
        blocks[0],
        serde_json::json!({ "items": [1, 2, 3] }),
    );
    let err = resolve_block_refs(
        doc.segments(),
        blocks[1],
        "SELECT * FROM x WHERE y = {{src.items}}",
        &empty_env(),
    )
    .expect_err("array values can't bind");
    assert!(err.contains("non-scalar"), "got: {err}");
}

#[test]
fn resolve_block_refs_unknown_alias_errors() {
    let md = "```db-postgres alias=q\nSELECT 1\n```\n";
    let doc = make_doc(md);
    let blocks = block_indices(&doc);
    let err = resolve_block_refs(
        doc.segments(),
        blocks[0],
        "SELECT * FROM x WHERE y = {{ghost.id}}",
        &empty_env(),
    )
    .expect_err("ghost alias has no upstream block");
    assert!(err.contains("ghost"), "got: {err}");
}

#[test]
fn resolve_block_refs_block_alias_wins_over_env_var_with_same_name() {
    // RF-04 precedence: when `{{X}}` could match both an upstream
    // block aliased `X` and an env var `X`, the block wins. Mirrors
    // desktop `references.ts` resolve order (block-first).
    let md = "```http alias=token\nGET /\n```\n\n```db-postgres alias=q\nSELECT 1\n```\n";
    let mut doc = make_doc(md);
    let blocks = block_indices(&doc);
    set_cache(&mut doc, blocks[0], serde_json::json!("from-block"));
    let env = env_map(&[("token", "from-env")]);
    let (sql, binds) =
        resolve_block_refs(doc.segments(), blocks[1], "SELECT {{token}}", &env).expect("resolves");
    assert_eq!(sql, "SELECT ?");
    assert_eq!(binds, vec![serde_json::json!("from-block")]);
}

#[test]
fn resolve_block_refs_preserves_query_when_no_refs_present() {
    let md = "```db-postgres alias=q\nSELECT 1\n```\n";
    let doc = make_doc(md);
    let blocks = block_indices(&doc);
    let (sql, binds) = resolve_block_refs(
        doc.segments(),
        blocks[0],
        "SELECT 1 FROM users LIMIT 10",
        &empty_env(),
    )
    .expect("resolves");
    assert_eq!(sql, "SELECT 1 FROM users LIMIT 10");
    assert!(binds.is_empty());
}

// ───────────── DB response shim (multi-statement) ─────────────
//
// For `db-*` blocks whose cached_result has the `{results: [...]}`
// shape, `{{alias.response.…}}` mirrors the desktop's
// `makeDbResponseView`:
//   - response.results / response.messages / response.stats: passthrough
//   - response.<N>: numeric shortcut → results[N]
//   - response.<col>: legacy → results[0].rows[0].<col>

#[test]
fn db_shim_legacy_response_col_resolves_first_row_first_result() {
    // `{{q.response.id}}` ≡ `results[0].rows[0].id` — pre-redesign
    // parity guarantee for notes that pre-date multi-result.
    let md = "```db-postgres alias=src\nSELECT 1\n```\n\n```db-postgres alias=q\nSELECT 1\n```\n";
    let mut doc = make_doc(md);
    let blocks = block_indices(&doc);
    set_cache(
        &mut doc,
        blocks[0],
        db_response(serde_json::json!([select_result(
            serde_json::json!([{ "id": 7, "name": "alice" }])
        ),])),
    );
    let (sql, binds) = resolve_block_refs(
        doc.segments(),
        blocks[1],
        "SELECT * FROM users WHERE id = {{src.response.id}}",
        &empty_env(),
    )
    .expect("resolves");
    assert_eq!(sql, "SELECT * FROM users WHERE id = ?");
    assert_eq!(binds, vec![serde_json::json!(7)]);
}

#[test]
fn db_shim_explicit_path_walks_results_array() {
    let md = "```db-postgres alias=src\nSELECT 1\n```\n\n```db-postgres alias=q\nSELECT 1\n```\n";
    let mut doc = make_doc(md);
    let blocks = block_indices(&doc);
    set_cache(
        &mut doc,
        blocks[0],
        db_response(serde_json::json!([select_result(
            serde_json::json!([{ "id": 7 }, { "id": 8 }])
        ),])),
    );
    let (_, binds) = resolve_block_refs(
        doc.segments(),
        blocks[1],
        "SELECT {{src.response.0.rows.1.id}}",
        &empty_env(),
    )
    .expect("resolves");
    assert_eq!(binds, vec![serde_json::json!(8)]);
}

#[test]
fn db_shim_numeric_shortcut_targets_second_result_set() {
    // `BEGIN; SELECT a; SELECT b; ROLLBACK;` → 4 results. The
    // numeric shortcut `response.2` grabs the *second* SELECT
    // without spelling out `results.2`.
    let md = "```db-postgres alias=src\nSELECT 1\n```\n\n```db-postgres alias=q\nSELECT 1\n```\n";
    let mut doc = make_doc(md);
    let blocks = block_indices(&doc);
    set_cache(
        &mut doc,
        blocks[0],
        db_response(serde_json::json!([
            serde_json::json!({ "kind": "mutation", "rows_affected": 0 }),
            select_result(serde_json::json!([{ "x": 1 }])),
            select_result(serde_json::json!([{ "y": 99 }])),
            serde_json::json!({ "kind": "mutation", "rows_affected": 0 }),
        ])),
    );
    let (_, binds) = resolve_block_refs(
        doc.segments(),
        blocks[1],
        "SELECT {{src.response.2.rows.0.y}}",
        &empty_env(),
    )
    .expect("resolves");
    assert_eq!(binds, vec![serde_json::json!(99)]);
}

#[test]
fn db_shim_passthrough_stats_returns_elapsed_ms() {
    let md = "```db-postgres alias=src\nSELECT 1\n```\n\n```db-postgres alias=q\nSELECT 1\n```\n";
    let mut doc = make_doc(md);
    let blocks = block_indices(&doc);
    set_cache(
        &mut doc,
        blocks[0],
        db_response(serde_json::json!([select_result(
            serde_json::json!([{ "id": 1 }])
        ),])),
    );
    let (_, binds) = resolve_block_refs(
        doc.segments(),
        blocks[1],
        "SELECT {{src.response.stats.elapsed_ms}}",
        &empty_env(),
    )
    .expect("resolves");
    assert_eq!(binds, vec![serde_json::json!(12)]);
}

#[test]
fn db_shim_mutation_rows_affected_via_explicit_path() {
    // Mutations have no `rows[]`, so legacy column shim doesn't
    // apply. Explicit `response.0.rows_affected` reads off the
    // result-set object.
    let md = "```db-postgres alias=src\nUPDATE foo SET x=1\n```\n\n```db-postgres alias=q\nSELECT 1\n```\n";
    let mut doc = make_doc(md);
    let blocks = block_indices(&doc);
    set_cache(
        &mut doc,
        blocks[0],
        db_response(serde_json::json!([
            serde_json::json!({ "kind": "mutation", "rows_affected": 7 }),
        ])),
    );
    let (_, binds) = resolve_block_refs(
        doc.segments(),
        blocks[1],
        "SELECT {{src.response.0.rows_affected}}",
        &empty_env(),
    )
    .expect("resolves");
    assert_eq!(binds, vec![serde_json::json!(7)]);
}

#[test]
fn db_shim_legacy_against_mutation_errors_clearly() {
    // `response.<col>` expects rows[0]; a mutation has none, so
    // the user sees a clear error instead of "column not found".
    let md = "```db-postgres alias=src\nUPDATE foo SET x=1\n```\n\n```db-postgres alias=q\nSELECT 1\n```\n";
    let mut doc = make_doc(md);
    let blocks = block_indices(&doc);
    set_cache(
        &mut doc,
        blocks[0],
        db_response(serde_json::json!([
            serde_json::json!({ "kind": "mutation", "rows_affected": 1 }),
        ])),
    );
    let err = resolve_block_refs(
        doc.segments(),
        blocks[1],
        "SELECT {{src.response.id}}",
        &empty_env(),
    )
    .expect_err("mutation has no rows");
    assert!(
        err.contains("rows") || err.contains("mutation"),
        "got: {err}"
    );
}

#[test]
fn db_shim_out_of_bounds_result_index_errors() {
    let md = "```db-postgres alias=src\nSELECT 1\n```\n\n```db-postgres alias=q\nSELECT 1\n```\n";
    let mut doc = make_doc(md);
    let blocks = block_indices(&doc);
    set_cache(
        &mut doc,
        blocks[0],
        db_response(serde_json::json!([select_result(
            serde_json::json!([{ "id": 1 }])
        ),])),
    );
    let err = resolve_block_refs(
        doc.segments(),
        blocks[1],
        "SELECT {{src.response.5.rows.0.id}}",
        &empty_env(),
    )
    .expect_err("only 1 result, idx 5 out of bounds");
    assert!(err.contains("out of bounds"), "got: {err}");
}

#[test]
fn db_shim_skipped_when_cached_lacks_results_array() {
    // Pre-redesign caches lack `{results: [...]}` — shim must
    // not engage so older notes still resolve via plain
    // dot-navigation.
    let md = "```db-postgres alias=src\nSELECT 1\n```\n\n```db-postgres alias=q\nSELECT 1\n```\n";
    let mut doc = make_doc(md);
    let blocks = block_indices(&doc);
    set_cache(&mut doc, blocks[0], serde_json::json!({ "id": 42 }));
    let (_, binds) = resolve_block_refs(
        doc.segments(),
        blocks[1],
        "SELECT {{src.response.id}}",
        &empty_env(),
    )
    .expect("resolves via legacy dot-nav");
    assert_eq!(binds, vec![serde_json::json!(42)]);
}

// ───────────── Executor params builder (timeout) ──────────────

#[test]
fn executor_params_includes_timeout_when_set() {
    let params = build_db_executor_params("conn-1", "SELECT 1", &[], 0, 100, Some(500), None);
    assert_eq!(params["timeout_ms"], 500);
}

#[test]
fn executor_params_emits_null_override_when_absent() {
    let params = build_db_executor_params("conn-1", "SELECT 1", &[], 0, 100, None, None);
    assert!(params["session_host_override"].is_null());
    assert!(params["session_port_override"].is_null());
}

#[test]
fn executor_params_forwards_session_override() {
    let ov = crate::session_overrides::ConnectionOverride {
        host: Some("db.staging".into()),
        port: Some(15432),
    };
    let params = build_db_executor_params("pg", "SELECT 1", &[], 0, 100, None, Some(&ov));
    assert_eq!(params["session_host_override"], "db.staging");
    assert_eq!(params["session_port_override"], 15432);
}

#[test]
fn executor_params_emits_null_timeout_when_absent() {
    // No fence token → field serializes as `null`. Executor falls
    // back to the connection's default timeout (and ultimately to
    // 30s).
    let params = build_db_executor_params("conn-1", "SELECT 1", &[], 0, 100, None, None);
    assert!(params["timeout_ms"].is_null());
}

#[test]
fn executor_params_passes_bind_values_through() {
    let binds = vec![serde_json::json!(7), serde_json::json!("alice")];
    let params = build_db_executor_params("conn-1", "SELECT ?, ?", &binds, 0, 50, None, None);
    assert_eq!(params["bind_values"][0], 7);
    assert_eq!(params["bind_values"][1], "alice");
    assert_eq!(params["fetch_size"], 50);
}

// ───────────── Alias edit ──────────────────────────────────

#[test]
fn alias_unique_passes_when_no_collision() {
    let md = "```http alias=existing\nGET /\n```\n\n```db-postgres\nSELECT 1\n```\n";
    let doc = make_doc(md);
    let blocks = block_indices(&doc);
    assert!(validate_alias_unique(&doc, blocks[1], "fresh_name").is_ok());
}

#[test]
fn alias_unique_blocks_collision_with_other_block() {
    // Silent shadowing would hide downstream `{{alias.path}}`
    // resolution from the second block.
    let md = "```http alias=existing\nGET /\n```\n\n```db-postgres\nSELECT 1\n```\n";
    let doc = make_doc(md);
    let blocks = block_indices(&doc);
    let err = validate_alias_unique(&doc, blocks[1], "existing").expect_err("collision must error");
    assert!(err.contains("existing"), "got: {err}");
}

#[test]
fn alias_unique_allows_same_block_keeping_its_own_alias() {
    // Self-comparison is skipped so editing-with-no-changes
    // doesn't hit a fake collision.
    let md = "```http alias=existing\nGET /\n```\n";
    let doc = make_doc(md);
    let blocks = block_indices(&doc);
    assert!(validate_alias_unique(&doc, blocks[0], "existing").is_ok());
}

// ───── Settings modal validation ─────

#[test]
fn parse_optional_u64_empty_returns_none() {
    // Empty input means "clear the field" — confirm path removes
    // the JSON key when this returns Ok(None).
    assert_eq!(parse_optional_u64(""), Ok(None));
}

#[test]
fn parse_optional_u64_accepts_zero_and_large() {
    assert_eq!(parse_optional_u64("0"), Ok(Some(0)));
    assert_eq!(parse_optional_u64("500"), Ok(Some(500)));
    assert_eq!(parse_optional_u64("4294967296"), Ok(Some(4_294_967_296)));
}

#[test]
fn parse_optional_u64_rejects_non_numeric() {
    assert!(parse_optional_u64("abc").is_err());
    assert!(parse_optional_u64("12.5").is_err());
    assert!(parse_optional_u64("-1").is_err());
    assert!(parse_optional_u64("3 4").is_err());
}

#[test]
fn db_settings_focus_cycle_db() {
    use crate::app::{DbSettingsState, SettingsField};
    use crate::vim::lineedit::LineEdit;
    let mut s = DbSettingsState {
        segment_idx: 0,
        fields: vec![
            SettingsField {
                label: "Limit",
                key: "limit",
                input: LineEdit::new(),
            },
            SettingsField {
                label: "Timeout",
                key: "timeout_ms",
                input: LineEdit::new(),
            },
        ],
        focus: 0,
    };
    s.focus_next();
    assert_eq!(s.focus, 1);
    s.focus_next();
    assert_eq!(s.focus, 0); // wraps
    s.focus_prev();
    assert_eq!(s.focus, 1); // wraps backwards
}

#[test]
fn preview_sql_collapses_whitespace_and_truncates() {
    let sql = "SELECT *\n  FROM users\nWHERE id = 1";
    assert_eq!(preview_sql(sql), "SELECT * FROM users WHERE id = 1");
}

#[test]
fn preview_sql_truncates_with_ellipsis() {
    let long_sql = "SELECT ".to_string() + &"col_name, ".repeat(40) + "FROM huge_table";
    let preview = preview_sql(&long_sql);
    assert!(
        preview.chars().count() <= 201,
        "got len {}",
        preview.chars().count()
    );
    assert!(preview.ends_with('…'));
}

#[test]
fn preview_sql_short_unchanged() {
    let sql = "SELECT 1";
    let preview = preview_sql(sql);
    assert_eq!(preview, "SELECT 1");
    assert!(!preview.ends_with('…'));
}

#[test]
fn db_settings_focus_cycle_http_is_noop() {
    use crate::app::{DbSettingsState, SettingsField};
    use crate::vim::lineedit::LineEdit;
    // HTTP modal has only timeout — Tab is a no-op.
    let mut s = DbSettingsState {
        segment_idx: 0,
        fields: vec![SettingsField {
            label: "Timeout",
            key: "timeout_ms",
            input: LineEdit::new(),
        }],
        focus: 0,
    };
    s.focus_next();
    assert_eq!(s.focus, 0);
    s.focus_prev();
    assert_eq!(s.focus, 0);
}

// ───────────── App-mutating cmds (cycle_display_mode / alias /
// run_explain / resolve_connection_id / load_active_env_vars) ─────────────

use crate::app::App;
use crate::buffer::Cursor;
use crate::config::Config;
use crate::pane::{Pane, TabState};
use crate::vault::ResolvedVault;
use httui_core::db::init_db;
use tempfile::TempDir;

async fn app_with_doc(md: &str) -> (App, TempDir, TempDir) {
    let data = TempDir::new().unwrap();
    let vault = TempDir::new().unwrap();
    let note = vault.path().join("note.md");
    std::fs::write(&note, md).unwrap();
    let pool = init_db(data.path()).await.unwrap();
    let resolved = ResolvedVault {
        vault: vault.path().to_path_buf(),
    };
    let mut app = App::new(Config::default(), resolved, pool);
    let doc = Document::from_markdown(md).unwrap();
    let pane = Pane::new(doc, note);
    app.tabs.tabs.clear();
    app.tabs.tabs.push(TabState::new(pane));
    app.tabs.active = 0;
    (app, data, vault)
}

fn place_cursor_in_first_block(app: &mut App) -> usize {
    let idx = app
        .document()
        .unwrap()
        .segments()
        .iter()
        .position(|s| matches!(s, Segment::Block(_)))
        .expect("block");
    app.document_mut().unwrap().set_cursor(Cursor::InBlock {
        segment_idx: idx,
        offset: 0,
    });
    idx
}

#[test]
fn resolve_connection_id_sync_matches_by_id_then_by_name() {
    use std::collections::HashMap;
    let mut names = HashMap::new();
    names.insert("uuid-1".into(), "prod-db".into());
    names.insert("uuid-2".into(), "dev-db".into());
    // raw is a known id key
    assert_eq!(resolve_connection_id_sync("uuid-1", &names), "uuid-1");
    // raw is a known name (case-insensitive)
    assert_eq!(resolve_connection_id_sync("PROD-DB", &names), "uuid-1");
    // raw is unknown — pass through verbatim
    assert_eq!(resolve_connection_id_sync("ghost", &names), "ghost");
}

#[tokio::test(flavor = "multi_thread")]
async fn cycle_display_mode_with_no_block_at_cursor_emits_status() {
    let (mut app, _d, _v) = app_with_doc("prose\n").await;
    cycle_display_mode(&mut app);
    assert!(app.status_message.is_some());
    let s = app.status_message.unwrap();
    assert!(s.text.contains("cursor"), "got {:?}", s.text);
}

#[tokio::test(flavor = "multi_thread")]
async fn cycle_display_mode_updates_block_display_and_status() {
    let md = "```db-sqlite alias=q\nSELECT 1;\n```\n";
    let (mut app, _d, _v) = app_with_doc(md).await;
    let idx = place_cursor_in_first_block(&mut app);
    cycle_display_mode(&mut app);
    let block = app
        .document()
        .unwrap()
        .segments()
        .get(idx)
        .and_then(|s| match s {
            Segment::Block(b) => Some(b.clone()),
            _ => None,
        })
        .unwrap();
    assert!(block.display_mode.is_some(), "display_mode set");
    assert!(app.status_message.is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn open_fence_edit_alias_no_block_emits_status() {
    let (mut app, _d, _v) = app_with_doc("prose\n").await;
    open_fence_edit_alias(&mut app);
    let s = app.status_message.unwrap();
    assert!(s.text.contains("cursor"), "got {:?}", s.text);
}

#[tokio::test(flavor = "multi_thread")]
async fn open_fence_edit_alias_opens_prompt_with_prefill() {
    let md = "```http alias=req\nGET /x\n```\n";
    let (mut app, _d, _v) = app_with_doc(md).await;
    place_cursor_in_first_block(&mut app);
    open_fence_edit_alias(&mut app);
    let Some(crate::modal::Modal::Prompt(kind, le)) = app.modal.as_ref() else {
        panic!("expected Prompt modal");
    };
    assert!(matches!(
        kind,
        crate::modal::PromptKind::FenceEditAlias { .. }
    ));
    assert_eq!(le.as_str(), "req");
    assert!(matches!(app.vim.mode, crate::vim::mode::Mode::FenceEdit));
}

#[tokio::test(flavor = "multi_thread")]
async fn confirm_fence_edit_with_no_prompt_resets_to_normal() {
    let (mut app, _d, _v) = app_with_doc("prose\n").await;
    confirm_fence_edit(&mut app);
    assert!(matches!(app.vim.mode, crate::vim::mode::Mode::Normal));
}

#[tokio::test(flavor = "multi_thread")]
async fn confirm_fence_edit_empty_alias_clears() {
    let md = "```http alias=req\nGET /x\n```\n";
    let (mut app, _d, _v) = app_with_doc(md).await;
    let idx = place_cursor_in_first_block(&mut app);
    open_fence_edit_alias(&mut app);
    if let Some(crate::modal::Modal::Prompt(_, le)) = app.modal.as_mut() {
        *le = crate::vim::lineedit::LineEdit::new();
    }
    confirm_fence_edit(&mut app);
    let block = match app.document().unwrap().segments().get(idx) {
        Some(Segment::Block(b)) => b.clone(),
        _ => panic!(),
    };
    assert!(block.alias.is_none(), "alias cleared");
    assert!(app.modal.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn confirm_fence_edit_new_alias_writes_back() {
    let md = "```http alias=req\nGET /x\n```\n";
    let (mut app, _d, _v) = app_with_doc(md).await;
    let idx = place_cursor_in_first_block(&mut app);
    open_fence_edit_alias(&mut app);
    if let Some(crate::modal::Modal::Prompt(_, le)) = app.modal.as_mut() {
        *le = crate::vim::lineedit::LineEdit::from_str("renamed");
    }
    confirm_fence_edit(&mut app);
    let block = match app.document().unwrap().segments().get(idx) {
        Some(Segment::Block(b)) => b.clone(),
        _ => panic!(),
    };
    assert_eq!(block.alias.as_deref(), Some("renamed"));
}

#[tokio::test(flavor = "multi_thread")]
async fn confirm_fence_edit_duplicate_alias_errors_and_keeps_prompt() {
    let md = "```http alias=a\nGET /x\n```\n\n```http alias=b\nGET /y\n```\n";
    let (mut app, _d, _v) = app_with_doc(md).await;
    // Place cursor on second block.
    let idxs: Vec<usize> = app
        .document()
        .unwrap()
        .segments()
        .iter()
        .enumerate()
        .filter_map(|(i, s)| matches!(s, Segment::Block(_)).then_some(i))
        .collect();
    let second = idxs[1];
    app.document_mut().unwrap().set_cursor(Cursor::InBlock {
        segment_idx: second,
        offset: 0,
    });
    open_fence_edit_alias(&mut app);
    if let Some(crate::modal::Modal::Prompt(_, le)) = app.modal.as_mut() {
        *le = crate::vim::lineedit::LineEdit::from_str("a"); // collision
    }
    confirm_fence_edit(&mut app);
    let s = app.status_message.expect("status set");
    assert!(s.text.contains("already used"), "got {:?}", s.text);
    assert!(app.modal.is_some(), "prompt must stay");
}

#[tokio::test(flavor = "multi_thread")]
async fn run_explain_with_no_block_at_cursor_emits_status() {
    let (mut app, _d, _v) = app_with_doc("prose\n").await;
    run_explain(&mut app);
    let s = app.status_message.expect("status set");
    assert!(s.text.contains("DB block"), "got {:?}", s.text);
}

#[tokio::test(flavor = "multi_thread")]
async fn run_explain_with_http_block_emits_not_a_db_block() {
    let md = "```http alias=a\nGET /x\n```\n";
    let (mut app, _d, _v) = app_with_doc(md).await;
    place_cursor_in_first_block(&mut app);
    run_explain(&mut app);
    let s = app.status_message.expect("status set");
    assert!(s.text.contains("not a DB"), "got {:?}", s.text);
}

#[tokio::test(flavor = "multi_thread")]
async fn load_active_env_vars_returns_none_when_no_active_env() {
    let data = TempDir::new().unwrap();
    let vault = TempDir::new().unwrap();
    let pool = init_db(data.path()).await.unwrap();
    let resolved = ResolvedVault {
        vault: vault.path().to_path_buf(),
    };
    let app = App::new(Config::default(), resolved, pool);
    let result = load_active_env_vars(&app.environments_store).await;
    assert!(result.is_none(), "no active env -> None");
}

#[tokio::test(flavor = "multi_thread")]
async fn resolve_connection_id_with_unknown_key_errors() {
    let data = TempDir::new().unwrap();
    let vault = TempDir::new().unwrap();
    let pool = init_db(data.path()).await.unwrap();
    let resolved = ResolvedVault {
        vault: vault.path().to_path_buf(),
    };
    let app = App::new(Config::default(), resolved, pool);
    let err = resolve_connection_id(&app.connections_store, "ghost")
        .await
        .unwrap_err();
    assert!(err.contains("not found"), "got {err:?}");
}
