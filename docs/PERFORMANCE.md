# Performance baselines

The numbers below are the user-felt latencies httui commits to.
Regressions against any target should fail CI before they reach
`main`. The harness that enforces this is the open follow-up of
this document; the targets themselves are the contract.

The five things users feel first:

| # | Metric | Target | Measured on | Notes |
|---|---|---|---|---|
| 1 | Cold start to first interactive frame | < 1.5s | M1 / 16 GB | From `open httui.app` to a clickable empty-state. |
| 2 | Idle memory footprint | < 200 MB | M1 / 16 GB | RSS after open with an empty vault, no chat session. |
| 3 | HTTP block overhead vs raw `curl` | < 50 ms | localhost loopback | `httui` minus `curl` median for the same request. |
| 4 | TOML parse on env switch | < 5 ms | typical vault (10 envs / 30 vars each) | Includes `.local` merge. |
| 5 | SQLite cache lookup | < 1 ms | `block_results` row hit | Indexed `(file_path, block_hash)` lookup. |

Numbers are reproducible: each measurement section below names the
files / functions to call and the command shape that gets you to
the same number. Drift > 20% against any target should be investigated
before the change ships.

## How to measure

### 1. Cold start to first interactive frame

```bash
# macOS — measure window-server first-paint via `osascript`
make build  # produces target/release/bundle/macos/httui.app
START=$(date +%s%3N)
open -W "target/release/bundle/macos/httui.app"
END=$(date +%s%3N)
echo "Cold start: $((END - START)) ms"
```

The measurement includes Tauri runtime init + Vite-built bundle parse
+ React first paint. Reduce by trimming the chat sidecar bootstrap
(Tauri `setup` hook) or by lazy-loading non-critical CodeMirror
extensions.

### 2. Idle memory footprint

```bash
# Open the app, close every other process, wait 30s for steady state.
ps -o rss= -p "$(pgrep -n httui)"  # in KB; divide by 1024 for MB
```

Watch for `vault_config::*Store` cache growth (each store keeps a
`RwLock<Option<Cached>>` per file). If a vault grows past ~100 envs
the per-env cache could push memory.

### 3. HTTP block overhead vs raw `curl`

```bash
# In a runbook, run a `GET http://localhost:8080/ping` block.
# Read `block_history.elapsed_ms` from the latest run.
# Compare to:
curl -s -o /dev/null -w "%{time_total}\n" http://localhost:8080/ping
```

Wrapper cost includes: ref resolution → `executor::http::execute_streamed`
→ `reqwest` build + send → result serialization. The `executor::http`
benches in `httui-core/src/executor/http/` cover the most expensive
paths.

### 4. TOML parse on env switch

```rust
// httui-core/src/vault_config/environments_store.rs::load_env
// Time `read_toml::<EnvFile>(path)` for the typical vault size.
```

Bench should include the `.local` override merge from
`vault_config::merge::load_with_local`. The store's mtime cache
short-circuits repeated reads, so the bench needs to invalidate the
cache between runs to measure actual disk + parse time.

### 5. SQLite cache lookup

```rust
// httui-core/src/block_results.rs::get_block_result
// Hot path: SELECT * FROM block_results WHERE file_path = ? AND block_hash = ?
```

The `(file_path, block_hash)` index from migration `001_initial.sql`
is what hits the < 1 ms target. If the table grows past ~100k rows
the WAL checkpoint cadence might dominate; the per-block-history
trim from migration `009_block_run_history.sql` keeps things bounded
in the run-history table but not in `block_results`. A cache-size cap
+ LRU eviction is on the table for v2.

## Open follow-ups

- **Bench harness** — not yet implemented. The targets
  above are committed; the criterion-based harness that enforces
  them in CI lands when the project picks a benchmarking crate.
  Two natural shapes:
  - `httui-core/benches/*.rs` with `criterion` + `cargo bench` →
    fits the standard Rust bench flow but needs a dev-dependency.
  - Plain `std::time::Instant` measurement loops in test files
    flagged `#[ignore]` → no new dep, but lacks criterion's
    statistical analysis.
  Either ship; both reproduce the numbers above.
- **CI integration** — run the bench on each PR;
  alert on >20% regression vs main. Owns the GH Actions side once
  the harness exists.
- **Real-hardware measurements** — the targets
  above are based on the current design; verifying them on the actual
  M1 / Linux / Windows targets is hardware-bound work for the
  user. The numbers should be re-confirmed pre-launch.
