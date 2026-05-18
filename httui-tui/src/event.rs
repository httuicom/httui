use crossterm::event::{Event as CtEvent, KeyEvent};
use futures::StreamExt;
use httui_core::db::schema_cache::SchemaEntry;
use httui_core::executor::db::types::DbResponse;
use httui_core::executor::http::types::HttpResponse;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::error::TuiResult;

/// All events the main loop reacts to.
///
/// Future variants for streaming block execution and file watcher events
/// land here without changing the dispatcher shape — / 22.
#[derive(Debug)]
#[allow(dead_code)] // Resize / Quit are wired but not yet consumed by the scaffold.
pub enum AppEvent {
    Key(KeyEvent),
    Resize(u16, u16),
    Tick,
    Quit,
    /// A backgrounded DB query (kicked off by `apply_run_block` /
    /// `load_more_db_block`) finished. The main loop folds the
    /// outcome back into the block at `segment_idx` and clears the
    /// app's `running_query` slot. `kind` distinguishes a fresh
    /// run from a paginated load-more so the merge logic picks the
    /// right strategy.
    DbBlockResult {
        segment_idx: usize,
        kind: DbBlockResultKind,
        outcome: Result<DbResponse, String>,
    },
    /// A backgrounded HTTP request finished. The main loop folds
    /// the response into `block.cached_result` and clears the
    /// `running_query` slot.
    HttpBlockResult {
        segment_idx: usize,
        outcome: Result<HttpResponse, String>,
    },
    /// A backgrounded schema introspection finished. Spawned by
    /// `App::ensure_schema_loaded` (typically right after the user
    /// confirms a connection in the picker). The main loop folds the
    /// result into `App.schema_cache` and clears the pending flag so
    /// retries are possible after an error.
    SchemaLoaded {
        connection_id: String,
        result: Result<Vec<SchemaEntry>, String>,
    },
    /// The async FTS5 index rebuild kicked off by `<C-f>` finished.
    /// Main loop flips `App.content_search_index_built = true` and
    /// runs the current query — the user may have typed during the
    /// build, so we re-query against the freshly populated index.
    /// Failure surfaces as a status error and clears the modal.
    ContentSearchIndexBuilt {
        result: Result<(), String>,
    },
    /// The filesystem watcher saw a modify/create event on the
    /// active document's path. The main loop compares disk content
    /// to the buffer; if equal, the event was our own write and we
    /// silently drop it. If different + clean, the buffer is
    /// reloaded silently. If different + dirty, a status warning
    /// surfaces (the user has unsaved changes that would be lost).
    FileChangedExternally {
        path: PathBuf,
    },
}

/// Why this DB result was produced — used by the main loop to pick
/// between "replace the cached_result" (fresh run), "append the
/// new page's rows" (load-more), and "merge into the plan slot
/// without touching results" (EXPLAIN).
#[derive(Debug)]
pub enum DbBlockResultKind {
    Run,
    LoadMore,
    /// `<C-x>` (EXPLAIN) — the query was wrapped in the dialect's
    /// EXPLAIN keyword. Result lands in `cached_result["plan"]`,
    /// not `cached_result.results`, so the original query's output
    /// isn't destroyed by inspecting its plan.
    Explain,
}

/// Spawns a background task that drains `crossterm`'s event stream and
/// emits a periodic `Tick` for animation/polling. The receiver is owned
/// by the main loop.
pub struct EventLoop {
    rx: mpsc::UnboundedReceiver<AppEvent>,
    tx: mpsc::UnboundedSender<AppEvent>,
    _handle: tokio::task::JoinHandle<()>,
}

impl EventLoop {
    pub fn start(tick_rate: Duration) -> TuiResult<Self> {
        let (tx, rx) = mpsc::unbounded_channel();
        let tx_clone = tx.clone();
        let handle = tokio::spawn(async move {
            let mut stream = crossterm::event::EventStream::new();
            let mut tick = tokio::time::interval(tick_rate);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    biased;
                    _ = tick.tick() => {
                        if tx_clone.send(AppEvent::Tick).is_err() {
                            break;
                        }
                    }
                    Some(Ok(ev)) = stream.next() => {
                        let mapped = match ev {
                            CtEvent::Key(k) => AppEvent::Key(k),
                            CtEvent::Resize(c, r) => AppEvent::Resize(c, r),
                            _ => continue,
                        };
                        if tx_clone.send(mapped).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        Ok(Self {
            rx,
            tx,
            _handle: handle,
        })
    }

    pub async fn next(&mut self) -> Option<AppEvent> {
        self.rx.recv().await
    }

    /// Hand a cloneable sender to dispatch handlers that need to
    /// inject events from spawned tasks (currently the async DB
    /// executor). The receiver continues to flow through `next()`.
    pub fn sender(&self) -> mpsc::UnboundedSender<AppEvent> {
        self.tx.clone()
    }
}
